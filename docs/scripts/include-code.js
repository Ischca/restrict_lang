#!/usr/bin/env node

/**
 * Script to process markdown files and include code examples
 * Usage: node include-code.js <input.md> <output.md>
 */

const fs = require('fs');
const path = require('path');

function processMarkdown(content, baseDir) {
    // Pattern to match include directives
    const includePattern = /<!--\s*include:\s*(.+?)\s*-->/g;
    const includeRangePattern = /<!--\s*include:\s*(.+?)\s*lines:\s*(\d+)-(\d+)\s*-->/g;
    
    // First, handle includes with line ranges
    content = content.replace(includeRangePattern, (match, filePath, startLine, endLine) => {
        const fullPath = path.join(baseDir, filePath.trim());
        try {
            const fileContent = fs.readFileSync(fullPath, 'utf8');
            const lines = fileContent.split('\n');
            const start = parseInt(startLine) - 1;
            const end = parseInt(endLine);
            const selectedLines = lines.slice(start, end).join('\n');
            
            // Detect language from file extension
            const ext = path.extname(filePath).slice(1);
            const lang = ext === 'rl' ? 'restrict' : ext;
            
            return '```' + lang + '\n' + selectedLines + '\n```';
        } catch (err) {
            console.error(`Error including file ${fullPath}:`, err.message);
            return match; // Keep original on error
        }
    });
    
    // Then handle simple includes
    content = content.replace(includePattern, (match, filePath) => {
        const fullPath = path.join(baseDir, filePath.trim());
        try {
            const fileContent = fs.readFileSync(fullPath, 'utf8');
            
            // Detect language from file extension
            const ext = path.extname(filePath).slice(1);
            const lang = ext === 'rl' ? 'restrict' : ext;
            
            return '```' + lang + '\n' + fileContent.trim() + '\n```';
        } catch (err) {
            console.error(`Error including file ${fullPath}:`, err.message);
            return match; // Keep original on error
        }
    });
    
    return content;
}

// Main execution
if (require.main === module) {
    const args = process.argv.slice(2);
    
    if (args.length < 2) {
        console.error('Usage: node include-code.js <input.md> <output.md>');
        process.exit(1);
    }
    
    const inputFile = args[0];
    const outputFile = args[1];
    const baseDir = path.dirname(inputFile);
    
    try {
        const content = fs.readFileSync(inputFile, 'utf8');
        const processed = processMarkdown(content, baseDir);
        fs.writeFileSync(outputFile, processed, 'utf8');
        console.log(`Processed ${inputFile} -> ${outputFile}`);
    } catch (err) {
        console.error('Error:', err.message);
        process.exit(1);
    }
}

module.exports = { processMarkdown };