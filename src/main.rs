use cedar_policy::{Authorizer, Context, Entities, PolicySet, Request, Schema};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct AuthzRequest {
    principal: String,
    action: String,
    resource: String,
    entities: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AuthzResponse {
    decision: String,
    diagnostics: Diagnostics,
}

#[derive(Debug, Serialize)]
struct Diagnostics {
    reason: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

struct CedarService {
    policy_set: PolicySet,
    schema: Option<Schema>,
}

impl CedarService {
    fn new(policy_path: &str, schema_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        println!("Loading policies from: {}", policy_path);
        println!("Loading schema from: {}", schema_path);

        let policy_src = fs::read_to_string(policy_path)
            .map_err(|e| format!("Failed to read policy file: {}", e))?;
        
        let policy_set = policy_src.parse::<PolicySet>()
            .map_err(|e| format!("Failed to parse policies: {}", e))?;

        let schema = if let Ok(schema_src) = fs::read_to_string(schema_path) {
            Some(Schema::from_json_str(&schema_src)
                .map_err(|e| format!("Failed to parse schema: {}", e))?)
        } else {
            println!("Warning: Schema file not found, proceeding without schema validation");
            None
        };

        println!("Cedar service initialized successfully");
        println!("Loaded {} policies", policy_set.policies().count());

        Ok(Self { policy_set, schema })
    }

    fn authorize(&self, req: AuthzRequest) -> Result<AuthzResponse, Box<dyn std::error::Error>> {
        println!("Authorization request - Principal: {}, Action: {}, Resource: {}", 
            req.principal, req.action, req.resource);

        // Parse entities
        let entities = if let Some(ref schema) = self.schema {
            Entities::from_json_value(req.entities, Some(schema))
                .map_err(|e| format!("Failed to parse entities: {}", e))?
        } else {
            Entities::from_json_value(req.entities, None)
                .map_err(|e| format!("Failed to parse entities: {}", e))?
        };

        // Parse principal, action, and resource
        let principal = req.principal.parse()
            .map_err(|e| format!("Failed to parse principal: {}", e))?;
        let action = req.action.parse()
            .map_err(|e| format!("Failed to parse action: {}", e))?;
        let resource = req.resource.parse()
            .map_err(|e| format!("Failed to parse resource: {}", e))?;

        // Create context (empty for now)
        let context = Context::empty();

        // Build Cedar request
        let cedar_request = if let Some(ref schema) = self.schema {
            Request::new(principal, action, resource, context, Some(schema))
                .map_err(|e| format!("Failed to create request: {}", e))?
        } else {
            Request::new(principal, action, resource, context, None)
                .map_err(|e| format!("Failed to create request: {}", e))?
        };

        // Evaluate authorization
        let authorizer = Authorizer::new();
        let response = authorizer.is_authorized(&cedar_request, &self.policy_set, &entities);

        // Build response
        let decision = match response.decision() {
            cedar_policy::Decision::Allow => "Allow",
            cedar_policy::Decision::Deny => "Deny",
        };

        // Get policy IDs that determined the decision
        let reason: Vec<String> = response
            .diagnostics()
            .reason()
            .map(|policy_id| policy_id.to_string())
            .collect();

        // Get any errors that occurred during evaluation
        let errors: Vec<String> = response
            .diagnostics()
            .errors()
            .map(|e| e.to_string())
            .collect();

        println!("Authorization decision: {} (reasons: {:?}, errors: {:?})", 
            decision, reason, errors);

        Ok(AuthzResponse {
            decision: decision.to_string(),
            diagnostics: Diagnostics { reason, errors },
        })
    }
}

async fn handle_request(
    req: hyper::Request<Body>,
    service: Arc<CedarService>,
) -> Result<Response<Body>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => {
            let health = HealthResponse {
                status: "healthy".to_string(),
            };
            let json = serde_json::to_string(&health).unwrap();
            Ok(Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(json))
                .unwrap())
        }

        (&Method::POST, "/authorize") => {
            let body_bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Failed to read request body: {}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(format!(r#"{{"error":"Failed to read body: {}"}}"#, e)))
                        .unwrap());
                }
            };

            match serde_json::from_slice::<AuthzRequest>(&body_bytes) {
                Ok(authz_req) => match service.authorize(authz_req) {
                    Ok(authz_response) => {
                        let json = serde_json::to_string(&authz_response).unwrap();
                        Ok(Response::builder()
                            .header("content-type", "application/json")
                            .body(Body::from(json))
                            .unwrap())
                    }
                    Err(e) => {
                        eprintln!("Authorization error: {}", e);
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .header("content-type", "application/json")
                            .body(Body::from(format!(r#"{{"error":"{}"}}"#, e)))
                            .unwrap())
                    }
                },
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("content-type", "application/json")
                        .body(Body::from(format!(r#"{{"error":"Invalid request: {}"}}"#, e)))
                        .unwrap())
                }
            }
        }

        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not found"))
            .unwrap()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy_path = std::env::var("CEDAR_POLICY_PATH")
        .unwrap_or_else(|_| "/app/policies/policy.cedar".to_string());
    let schema_path = std::env::var("CEDAR_SCHEMA_PATH")
        .unwrap_or_else(|_| "/app/policies/schema.cedarschema.json".to_string());
    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8181".to_string());

    let service = Arc::new(CedarService::new(&policy_path, &schema_path)?);

    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|e| format!("Invalid bind address: {}", e))?;

    let make_svc = make_service_fn(move |_| {
        let service = Arc::clone(&service);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, Arc::clone(&service))
            }))
        }
    });

    println!("Cedar Local Agent listening on {}", addr);
    Server::bind(&addr).serve(make_svc).await?;

    Ok(())
}