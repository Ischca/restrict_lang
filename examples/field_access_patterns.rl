// Field Access Patterns in Restrict Language
// Demonstrates different approaches to handling multiple field access

// =============================================================================
// COPYABLE RECORDS: All fields are primitives
// =============================================================================

record Point2D { x: Float64, y: Float64 }
record RGB { r: Int32, g: Int32, b: Int32 }

fn copyable_field_demo() {
    val point = Point2D { x = 3.0, y = 4.0 }
    
    // Multiple field access works naturally - all fields are copyable
    val x = point.x                            // OK: Float64 is copyable
    val y = point.y                            // OK: Float64 is copyable  
    val distance = sqrt(point.x^2 + point.y^2) // OK: can reuse point
    
    val color = RGB { r = 255, g = 128, b = 64 }
    val brightness = (color.r + color.g + color.b) / 3  // OK: all Int32
}

// =============================================================================
// MIXED RECORDS: Some copyable, some affine fields  
// =============================================================================

record User { 
    id: Int32,        // Copyable primitive
    name: String,     // Affine (heap-allocated)
    email: String     // Affine (heap-allocated)
}

record FileInfo {
    size: Int32,      // Copyable primitive
    path: String,     // Affine (heap-allocated)  
    handle: FileDesc  // Affine (resource handle)
}

fn mixed_field_demo() {
    val user = User { id = 123, name = "Alice", email = "alice@example.com" }
    
    // Can access copyable fields multiple times
    val id1 = user.id      // OK: Int32 is copyable, doesn't consume user
    val id2 = user.id      // OK: can access copyable field again
    val is_admin = user.id == 1  // OK: still can use user.id
    
    // But accessing affine field consumes the record
    val name = user.name   // Consumes user (String is affine)
    // val email = user.email  // ERROR: user already consumed
    
    // BETTER: Use destructuring for multiple affine fields
    val user2 = User { id = 456, name = "Bob", email = "bob@example.com" }
    val User { id, name, email } = user2  // Single consumption, access all fields
    val greeting = "Hello " ++ name ++ " (ID: " ++ toString(id) ++ ")"
}

// =============================================================================
// RESOURCE RECORDS: All fields are affine
// =============================================================================

record Database { 
    connection: DbConnection,   // Affine resource
    transaction: Transaction,   // Affine resource
    logger: Logger             // Affine resource
}

fn resource_field_demo() {
    val db = Database {
        connection = connect("localhost:5432"),
        transaction = begin_transaction(),
        logger = create_logger()
    }
    
    // For resource records, destructuring is the only practical pattern
    val Database { connection, transaction, logger } = db
    
    // Now can use all resources independently
    connection |> execute_query("SELECT * FROM users")
    transaction |> commit
    logger |> log_info("Transaction completed")
}

// =============================================================================
// NESTED COPYABLE RECORDS
// =============================================================================

record Dimensions { width: Float64, height: Float64, depth: Float64 }
record BoundingBox { 
    min: Point2D,     // All Point2D fields are copyable
    max: Point2D,     // So Point2D itself could be copyable
    dimensions: Dimensions  // All Dimensions fields are copyable
}

fn nested_copyable_demo() {
    val bbox = BoundingBox {
        min = Point2D { x = 0.0, y = 0.0 },
        max = Point2D { x = 10.0, y = 15.0 },
        dimensions = Dimensions { width = 10.0, height = 15.0, depth = 5.0 }
    }
    
    // If all nested records have only copyable fields,
    // nested access could work without consumption
    val width = bbox.dimensions.width    // Could work if Dimensions is copyable
    val area = bbox.dimensions.width * bbox.dimensions.height
    val min_x = bbox.min.x               // Could work if Point2D is copyable
}

// =============================================================================
// PROTOTYPE-BASED APPROACH: Advanced pattern using view prototypes
// =============================================================================

record ComplexData {
    metrics: DataMetrics,     // Mix of copyable/affine
    resources: ResourceSet,   // All affine
    config: Configuration     // All copyable
}

// View prototype for safe, repeated access to copyable parts
frozen record ComplexDataView {
    get_metric_count: fn() -> Int32,
    get_config_timeout: fn() -> Float64,
    get_total_size: fn() -> Int32
}

impl ComplexData {
    fn create_view(self) -> (ComplexDataView, ComplexData) {
        val view = ComplexDataView {
            get_metric_count: || self.metrics.count,    // Assuming count is Int32
            get_config_timeout: || self.config.timeout, // Assuming timeout is Float64  
            get_total_size: || self.metrics.count * self.config.batch_size
        }
        (view, self)
    }
}

fn prototype_view_demo() {
    val data = ComplexData { /* ... */ }
    val (view, data_back) = data.create_view()
    
    // Can query copyable data multiple times through view
    val count = view.get_metric_count()
    val timeout = view.get_config_timeout()
    val size = view.get_total_size()
    
    // Original data still available for final consumption
    val ComplexData { metrics, resources, config } = data_back
}

// =============================================================================
// COMPILER GUIDANCE: Examples that should suggest better patterns
// =============================================================================

fn needs_improvement() {
    val user = User { id = 123, name = "Alice", email = "alice@example.com" }
    
    // This pattern should trigger compiler hint:
    // "Consider destructuring: val User { id, name, email } = user"
    val name = user.name      // Consumes user
    // val email = user.email // Would fail - compiler suggests destructuring
}

// =============================================================================
// MIGRATION EXAMPLES: Moving from borrowing-style to Restrict-style
// =============================================================================

// Instead of this (borrowing-style thinking):
fn calculate_distance_bad(p1: Point2D, p2: Point2D) -> Float64 {
    // Thinking: "I need to access multiple fields"
    val dx = p1.x - p2.x  // Consumes p1 
    // val dy = p1.y - p2.y  // ERROR: p1 already consumed
}

// Do this (Restrict-style):
fn calculate_distance_good(p1: Point2D, p2: Point2D) -> Float64 {
    // Pattern 1: Destructure first
    val Point2D { x: x1, y: y1 } = p1
    val Point2D { x: x2, y: y2 } = p2
    val dx = x1 - x2
    val dy = y1 - y2
    sqrt(dx * dx + dy * dy)
}

// Or even better (if Point2D fields are all copyable):
fn calculate_distance_best(p1: Point2D, p2: Point2D) -> Float64 {
    // Pattern 2: Direct copyable field access
    val dx = p1.x - p2.x  // OK: Float64 is copyable
    val dy = p1.y - p2.y  // OK: Float64 is copyable
    sqrt(dx * dx + dy * dy)
}