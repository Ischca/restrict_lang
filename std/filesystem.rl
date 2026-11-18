// Standard Library: FileSystem capability profile
//
// This file is a planned capability-profile specification and is not an active
// pure-Restrict module in this release.
// Importing `std.filesystem` fails with an explicit capability-profile
// diagnostic.
//
// File handles, path reads, path writes, and close operations require explicit
// host filesystem capabilities outside the default pure-Restrict surface.
