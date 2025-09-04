//! Real-World Linear Threading Examples
//! 
//! Demonstrates practical usage patterns that show both elegance and limitations
//! of the Linear Threading with Residual Rebinding proposal.

// === WEB SERVER REQUEST HANDLING ===

record HttpRequest {
    method: String,
    path: String,
    headers: Map<String, String>,
    body: Vec<u8>,
    remote_addr: String,
    timestamp: Timestamp
}

record HttpResponse {
    status: Int32,
    headers: Map<String, String>,
    body: Vec<u8>
}

// Traditional approach - violates affine types
fun handleRequestTraditional: (req: HttpRequest) -> HttpResponse = {
    val method = req.method;     // req partially consumed
    val path = req.path;         // req further consumed - INVALID
    val headers = req.headers;   // req cannot be used again - INVALID
    
    // This violates affine constraints
    processRequest(method, path, headers)
}

// Linear Threading approach - preserves affine semantics
fun handleRequestLinear: (req: HttpRequest) -> (HttpResponse, AuditLog) = {
    consume req as { method, path, headers, remainder };
    
    val response = match (method, path) {
        ("GET", "/api/users") => {
            consume remainder as { remote_addr, audit_info };
            val users = Database.getUsers();
            HttpResponse {
                status: 200,
                headers: Map.empty(),
                body: users.toJson()
            }
        },
        ("POST", "/api/users") => {
            consume remainder as { body, audit_info };
            val user = User.fromJson(body);
            Database.createUser(user);
            HttpResponse {
                status: 201,
                headers: Map.empty(), 
                body: "Created".toBytes()
            }
        },
        _ => {
            HttpResponse {
                status: 404,
                headers: Map.empty(),
                body: "Not Found".toBytes()
            }
        }
    };
    
    val audit = AuditLog {
        request_info: remainder,  // Contains remaining fields
        response_status: response.status
    };
    
    (response, audit)
}

// === DATABASE CONNECTION POOLING ===

record DatabaseConnection<~conn> {
    socket: TcpStream<~conn>,
    transaction_state: TransactionState,
    connection_id: UUID,
    created_at: Timestamp,
    stats: ConnectionStats
}

record ConnectionPool<~pool> {
    active_connections: Vec<DatabaseConnection<~pool>>,
    config: PoolConfig,
    metrics: PoolMetrics
}

// Efficient connection borrowing with linear threading
fun borrowConnection<~pool, ~conn>: (
    pool: ConnectionPool<~pool>
) -> (DatabaseConnection<~conn>, ConnectionPool<~pool>) 
where ~conn within ~pool = {
    
    consume pool as { active_connections, remainder };
    
    active_connections match {
        [] => {
            // No connections available - create new one
            val new_conn = DatabaseConnection.create();
            val updated_pool = clone remainder with {
                active_connections: [],
                metrics: remainder.metrics.incrementCreated()
            };
            (new_conn, updated_pool)
        },
        [head | tail] => {
            // Borrow existing connection
            consume head as { socket, connection_id, conn_remainder };
            
            val borrowed_conn = DatabaseConnection {
                socket,
                connection_id,
                transaction_state: TransactionState.Active,
                created_at: conn_remainder.created_at,
                stats: conn_remainder.stats.incrementBorrowed()
            };
            
            val updated_pool = clone remainder with {
                active_connections: tail,
                metrics: remainder.metrics.incrementBorrowed()
            };
            
            (borrowed_conn, updated_pool)
        }
    }
}

// === CONFIGURATION MANAGEMENT ===

record AppConfig {
    database: DatabaseConfig,
    server: ServerConfig, 
    logging: LogConfig,
    features: FeatureFlags,
    secrets: SecretConfig
}

record DatabaseConfig {
    host: String,
    port: Int32,
    username: String,
    password: String,
    pool_size: Int32
}

// Safe config processing - secrets remain isolated
fun initializeApp: (config: AppConfig) -> (Application, SecretConfig) = {
    consume config as { database, server, logging, features, secrets };
    
    // Process non-sensitive config
    val db_pool = DatabasePool.create(database);
    val http_server = HttpServer.create(server);
    val logger = Logger.create(logging);
    
    val app = Application {
        database: db_pool,
        server: http_server,
        logger: logger,
        features: features
    };
    
    // Return secrets separately for secure handling
    (app, secrets)
}

// === STREAMING DATA PROCESSING ===

record StreamEvent {
    id: UUID,
    timestamp: Timestamp,
    event_type: String,
    payload: Vec<u8>,
    metadata: Map<String, String>,
    source_info: SourceInfo
}

record ProcessedEvent {
    id: UUID,
    processed_at: Timestamp,
    result: ProcessingResult,
    original_metadata: Map<String, String>
}

// Efficient stream processing with memory optimization
fun processEventStream: (events: Vec<StreamEvent>) -> Vec<ProcessedEvent> = {
    events |> map |> { event =>
        consume event as { id, timestamp, event_type, payload, remainder };
        
        val result = match event_type {
            "user_action" => {
                consume remainder as { metadata, user_info };
                UserActionProcessor.process(payload, metadata)
            },
            "system_event" => {
                consume remainder as { source_info, sys_info };
                SystemEventProcessor.process(payload, source_info)
            },
            _ => ProcessingResult.Skipped
        };
        
        ProcessedEvent {
            id: id,
            processed_at: now(),
            result: result,
            original_metadata: remainder.metadata  // Preserved from residual
        }
    }
}

// === FINANCIAL TRANSACTION PROCESSING ===

record Transaction {
    id: TransactionId,
    from_account: AccountId,
    to_account: AccountId,
    amount: Decimal,
    currency: Currency,
    timestamp: Timestamp,
    metadata: TransactionMetadata,
    signatures: Vec<Signature>
}

record TransactionMetadata {
    reference: String,
    description: String,
    category: String,
    tags: Vec<String>,
    risk_score: f64
}

// Secure transaction processing with audit trail
fun processTransaction: (tx: Transaction) -> (TransactionResult, AuditRecord) = {
    consume tx as { id, from_account, to_account, amount, currency, remainder };
    
    // Basic validation using consumed fields
    val validation_result = validateTransactionBasics(from_account, to_account, amount, currency);
    
    match validation_result {
        ValidationResult.Valid => {
            consume remainder as { signatures, metadata, audit_info };
            
            // Verify signatures
            val sig_valid = signatures |> all |> { sig => 
                Crypto.verify(sig, (id, from_account, to_account, amount))
            };
            
            if sig_valid {
                // Process the transaction
                val result = AccountingEngine.transfer(from_account, to_account, amount, currency);
                
                val audit = AuditRecord {
                    transaction_id: id,
                    metadata: metadata,
                    timestamp: audit_info.timestamp,
                    result: result,
                    signatures_verified: true
                };
                
                (result, audit)
            } else {
                val audit = AuditRecord {
                    transaction_id: id,
                    metadata: metadata,
                    timestamp: audit_info.timestamp,
                    result: TransactionResult.SignatureFailure,
                    signatures_verified: false
                };
                
                (TransactionResult.SignatureFailure, audit)
            }
        },
        ValidationResult.Invalid(reason) => {
            val audit = AuditRecord {
                transaction_id: id,
                metadata: remainder.metadata,
                timestamp: remainder.timestamp,
                result: TransactionResult.ValidationFailure(reason),
                signatures_verified: false
            };
            
            (TransactionResult.ValidationFailure(reason), audit)
        }
    }
}

// === PROBLEMS AND LIMITATIONS ===

// Problem 1: Complex nested consumption becomes unwieldy
record DeeplyNested {
    level1: Level1,
    other_fields: String
}

record Level1 {
    level2: Level2,
    level1_data: String
}

record Level2 {
    target_field: String,
    level2_data: String
}

// This becomes awkward with deep nesting
fun extractDeepField: (nested: DeeplyNested) -> String = {
    consume nested as { level1, nested_remainder };
    consume level1 as { level2, level1_remainder };
    consume level2 as { target_field, level2_remainder };
    
    // Now we have target_field but many leftover pieces
    // How do we reconstruct if needed?
    target_field
}

// Problem 2: Partial consumption with shared data
record SharedResource {
    shared_data: Arc<String>,  // Reference-counted shared data
    unique_id: UUID,
    metadata: ResourceMetadata
}

fun problematicSharing: (resource: SharedResource) -> () = {
    consume resource as { shared_data, remainder };
    
    // Problem: shared_data is moved, but remainder might need access
    // to the same Arc<String>. This violates sharing semantics.
    
    // In traditional systems: both would share the Arc
    // With linear threading: Arc is moved, remainder loses access
}

// Problem 3: Performance with large objects
record LargeDataSet {
    primary_data: Vec<i64>,      // 10MB array
    metadata: DataMetadata,      // Small metadata
    checksum: u64                // Single field
}

fun performanceIssue: (dataset: LargeDataSet) -> u64 = {
    consume dataset as { checksum, remainder };
    
    // Problem: To preserve remainder, the entire 10MB primary_data
    // must remain allocated even though we only wanted checksum
    // This defeats memory efficiency goals
    
    checksum
}

// === PROPOSED SOLUTIONS ===

// Solution 1: Selective consumption with hints
fun efficientExtraction: (dataset: LargeDataSet) -> u64 = {
    consume dataset as { 
        checksum,
        // Hint: we don't need remainder, allow cleanup
        remainder @drop  
    };
    
    // remainder is not bound, allows immediate cleanup
    checksum
}

// Solution 2: Staged consumption for complex cases
fun stagedConsumption: (nested: DeeplyNested) -> String = {
    // Stage 1: Extract level1
    consume nested as { level1, _ @drop };
    
    // Stage 2: Extract level2  
    consume level1 as { level2, _ @drop };
    
    // Stage 3: Extract target
    consume level2 as { target_field, _ @drop };
    
    target_field
}

// Solution 3: Reference splitting for shared data
fun referenceSplitting: (resource: SharedResource) -> (Arc<String>, UUID) = {
    consume resource as { 
        shared_data,
        unique_id,
        remainder @drop
    };
    
    // Arc automatically handles reference counting
    // No linear threading conflicts with shared ownership
    (shared_data, unique_id)
}