# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust web scraping application that fetches novel content from docln.net and converts it to EPUB format. The application supports both original novels (sang-tac) and AI-translated novels (ai-dich).

## Architecture Overview

The application follows a single-file architecture with the main functionality in `src/main.rs`:

### Core Structures
- **NovelInfo**: Main data structure containing novel metadata, volumes, and chapters
- **Volume**: Represents a novel volume with chapters and cover image
- **Chapter**: Contains chapter content, illustrations, and XHTML path for EPUB
- **DoclnCrawler**: Main crawler implementation handling HTTP requests and parsing

### Key Components

1. **Web Scraping Layer** (`DoclnCrawler`):
   - Uses `reqwest` for HTTP requests with custom user agent
   - Uses `scraper` crate for HTML parsing
   - Implements rate limiting (500ms delays between requests)

2. **Content Processing**:
   - Downloads chapter content and converts to XHTML format
   - Handles illustration downloading and path replacement
   - Organizes content by volumes with proper directory structure

3. **EPUB Generation**:
   - Creates EPUB 2.0 compliant directory structure
   - Generates `content.opf` metadata file
   - Generates `toc.ncx` navigation file
   - Organizes images and text content in `OEBPS/` directory

### Directory Structure for EPUB Output
```
epub_{novel_id}/
├── mimetype                    # EPUB MIME type
├── META-INF/
│   └── container.xml          # EPUB container
└── OEBPS/                     # OEBPS content
    ├── content.opf            # Package metadata
    ├── toc.ncx                # Navigation
    ├── images/                # Images organized by volume
    │   ├── volume_001/
    │   │   ├── chapter_001/   # Chapter illustrations
    │   │   └── cover.jpg      # Volume cover
    └── text/                  # XHTML content
        └── volume_001/
            └── chapter_001.xhtml
```

### External Dependencies
- **reqwest**: HTTP client for web requests
- **tokio**: Async runtime
- **scraper**: HTML parsing
- **serde**: JSON serialization
- **uuid**: Unique identifier generation
- **chrono**: Date/time handling

## Important Implementation Details

1. **Rate Limiting**: The crawler implements 500ms delays between requests to avoid overwhelming the server
2. **Image Handling**: Downloads and organizes illustrations by volume/chapter structure
3. **Error Handling**: Continues processing even if individual chapters fail
4. **User Agent**: Uses a browser-like user agent to avoid blocking
5. **Timeout**: 30-second timeout for HTTP requests
6. **Content Organization**: Groups content by volumes with sequential numbering

## Usage Pattern

The application runs interactively, prompting users to:
1. Select novel category (original or AI-translated)
2. Enter novel ID
3. Process and display results
4. Option to continue with another novel