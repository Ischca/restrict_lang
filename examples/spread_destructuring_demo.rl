// Spread Destructuring Demo - Restrict Language
// Demonstrating the power of spread destructuring in a prototype-based language
// Using OSV syntax throughout with affine type benefits

// Basic spread destructuring with records
// The spread (...) operator allows differential programming - 
// updating only what's needed while preserving the rest
let userProfile = {
    name: "Alice",
    age: 30,
    email: "alice@example.com",
    preferences: {
        theme: "dark",
        notifications: true,
        language: "en"
    }
};

// OSV syntax: record operation.clone creates new instance
// Spread destructuring allows selective updates without borrowing issues
let updatedProfile = { 
    ...userProfile, 
    age: 31,
    preferences: {
        ...userProfile.preferences,
        theme: "light"
    }
} userProfile.clone;

// Nested record destructuring with spread
// Demonstrates prototype chain preservation with differential updates
let baseConfig = {
    server: {
        host: "localhost",
        port: 8080,
        ssl: false
    },
    database: {
        url: "postgres://localhost",
        maxConnections: 10
    },
    logging: {
        level: "info",
        format: "json"
    }
};

// OSV: merging configs while maintaining prototype relationships
let productionConfig = {
    ...baseConfig,
    server: {
        ...baseConfig.server,
        host: "prod.example.com",
        ssl: true
    },
    logging: {
        ...baseConfig.logging,
        level: "warn"
    }
} baseConfig.clone;

// Function parameter spread destructuring (OSV style)
// Functions receive objects and destructure with spread
fun updateUserSettings(settings) {
    // Pattern match with spread to extract known fields
    let { theme, notifications, ...otherSettings } = settings;
    
    // OSV: new settings object.create with spread
    {
        theme: theme |> validateTheme,
        notifications: notifications,
        ...otherSettings
    } settings.merge
}

// Advanced: Combining spread with pattern matching
// Demonstrating affine type safety with spread operations
fun processApiResponse(response) {
    response match {
        // Success case with spread destructuring
        { status: "success", data: { user, ...metadata }, ...extra } => {
            // OSV: create success result with spread
            {
                user: user,
                timestamp: getCurrentTime(),
                ...metadata,
                ...extra
            } createSuccessResult
        },
        
        // Error case preserving all error information
        { status: "error", ...errorDetails } => {
            // Spread preserves all error context
            {
                ...errorDetails,
                handledAt: getCurrentTime(),
                retryable: errorDetails.code != 404
            } createErrorResult
        },
        
        // Default case with complete spread
        unknownResponse => {
            {
                ...unknownResponse,
                status: "unknown",
                processedAt: getCurrentTime()
            } createUnknownResult
        }
    }
}

// Real-world example: User profile updates
// Shows how spread avoids common borrowing and mutation issues
let userRepository = {
    updateProfile: fun(userId, updates) {
        // OSV: current profile retrieval
        let currentProfile = userId this.getProfile;
        
        // Spread destructuring for safe updates
        // Affine types ensure we don't accidentally reuse currentProfile
        let { password, ...publicUpdates } = updates;
        
        // OSV: create new profile preserving existing data
        let newProfile = {
            ...currentProfile,
            ...publicUpdates,
            updatedAt: getCurrentTime(),
            version: currentProfile.version + 1
        } currentProfile.clone;
        
        // Password handled separately for security
        let finalProfile = password match {
            Some(newPassword) => {
                {
                    ...newProfile,
                    passwordHash: newPassword this.hashPassword
                } newProfile.clone
            },
            None => newProfile
        };
        
        // OSV: save the updated profile
        finalProfile userId this.saveProfile
    }
};

// Configuration merging patterns
// Demonstrates prototype-based inheritance with spread
let defaultAppConfig = {
    api: {
        baseUrl: "http://localhost:3000",
        timeout: 5000,
        retries: 3
    },
    ui: {
        theme: "system",
        animations: true,
        density: "normal"
    },
    features: {
        analytics: false,
        experiments: false
    }
} freeze; // Freeze the prototype to prevent accidental mutations

// Environment-specific configs inherit from default
let developmentConfig = {
    ...defaultAppConfig,
    api: {
        ...defaultAppConfig.api,
        baseUrl: "http://localhost:3001"
    },
    features: {
        ...defaultAppConfig.features,
        analytics: true
    }
} defaultAppConfig.clone;

let productionConfig = {
    ...defaultAppConfig,
    api: {
        ...defaultAppConfig.api,
        baseUrl: "https://api.production.com",
        timeout: 10000
    },
    features: {
        ...defaultAppConfig.features,
        analytics: true,
        experiments: true
    }
} defaultAppConfig.clone;

// Differential record updates - memory efficient prototype pattern
// Only stores differences from the base prototype
fun createUserVariant(baseUser, customizations) {
    // OSV: spread creates minimal differential object
    {
        ...baseUser,
        ...customizations,
        variantCreatedAt: getCurrentTime()
    } baseUser.clone
}

// Advanced: Spread with temporal affine types
// Demonstrates resource-safe operations with spread
fun processFileWithBackup(filePath) {
    with fileHandle = filePath openFile {
        // Read current file metadata
        let metadata = fileHandle.getMetadata;
        
        // Create backup info with spread
        let backupInfo = {
            ...metadata,
            originalPath: filePath,
            backupCreatedAt: getCurrentTime(),
            backupType: "automatic"
        } metadata.clone;
        
        // OSV: backup file creation with spread data
        backupInfo (filePath + ".backup") this.createBackupFile;
        
        // Process original file...
        fileHandle this.processFile
    } // fileHandle automatically cleaned up
}

// Utility functions for demonstration
fun validateTheme(theme) {
    theme match {
        "light" | "dark" | "system" => theme,
        _ => "system"
    }
}

fun getCurrentTime() {
    // Implementation would return current timestamp
    1640995200 // Placeholder timestamp
}

fun createSuccessResult(data) {
    { status: "processed", result: data }
}

fun createErrorResult(error) {
    { status: "failed", error: error }
}

fun createUnknownResult(response) {
    { status: "unknown", raw: response }
}

// Main demonstration function showing practical usage
fun demonstrateSpreadPatterns() {
    // Basic spread usage
    let originalData = { a: 1, b: 2, c: 3 };
    let extendedData = { ...originalData, d: 4 } originalData.clone;
    
    // Nested spread with selective updates
    let complexObj = {
        config: { debug: true, verbose: false },
        data: { items: [], count: 0 }
    };
    
    let updatedObj = {
        ...complexObj,
        config: {
            ...complexObj.config,
            verbose: true
        }
    } complexObj.clone;
    
    // Function parameter spread
    let mergeConfigs = fun(base, overrides) {
        { ...base, ...overrides } base.clone
    };
    
    // Return demonstration results
    {
        extended: extendedData,
        updated: updatedObj,
        merger: mergeConfigs
    }
}