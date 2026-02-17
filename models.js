[
    {
        "name": "LFM2.5-1.2B-Thinking",
        "provider": "LiquidAI",
        "description": "General-purpose text-only model with stellar performance in instruction following, tool-use, and math. Recommended for agentic tasks, data extraction, RAG. Not recommended for knowledge-intensive tasks or programming.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "January 21"
    },
    {
        "name": "LFM2-2.6B-Transcript",
        "provider": "LiquidAI",
        "description": "Based on LFM2-2.6B. Designed for private, on-device meeting summarization.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "2.6B",
        "updated": "January 7"
    },
    {
        "name": "LFM2.5-VL-1.6B",
        "provider": "LiquidAI",
        "description": "Refreshed vision-language model. Built on LFM2.5-1.2B-Base, tuned for real-world performance, and on-device deployment.",
        "modalities": ["text", "image"],
        "quantization": ["F16", "Q4_0", "Q8_0"],
        "parameters": "1.6B",
        "updated": "January 6"
    },
    {
        "name": "LFM2.5-Audio-1.5B",
        "provider": "LiquidAI",
        "description": "End-to-end multimodal speech and text model. No separate ASR or TTS needed. Enables low-latency, real-time conversation with performance matching larger models.",
        "modalities": ["text", "audio"],
        "quantization": ["F16", "Q4_0", "Q8_0"],
        "parameters": "1.5B",
        "updated": "January 6"
    },
    {
        "name": "LFM2.5-1.2B-JP",
        "provider": "LiquidAI",
        "description": "General-purpose text-only model optimized for Japanese. Ideal for applications where cultural and linguistic nuance are important.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "January 6"
    },
    {
        "name": "LFM2.5-1.2B-Instruct",
        "provider": "LiquidAI",
        "description": "General-purpose text-only model, part of the on-device LFM2.5 hybrid model family.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "January 6"
    },
    {
        "name": "LFM2-2.6B-Exp",
        "provider": "LiquidAI",
        "description": "Experimental checkpoint using reinforcement learning. Excels at instruction following, knowledge, math. IFBench score surpasses DeepSeek R1-0528.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "2.6B",
        "updated": "December 26"
    },
    {
        "name": "LFM2-VL-3B",
        "provider": "LiquidAI",
        "description": "Newest multimodal model in LFM2-VL series. Handles text and images at variable resolutions. Enhanced reasoning and visual understanding with efficiency.",
        "modalities": ["text", "image"],
        "quantization": ["F16", "Q8_0"],
        "parameters": "3B",
        "updated": "October 22"
    },
    {
        "name": "LFM2-350M-PII-Extract-JP",
        "provider": "LiquidAI",
        "description": "Optimized for PII extraction from Japanese. Outputs JSON for use in redacting personal info from various documents on-device.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "350M",
        "updated": "October 11"
    },
    {
        "name": "LFM2-2.6B",
        "provider": "LiquidAI",
        "description": "Next-gen hybrid model for edge and on-device AI. Sets new standard for quality, speed, efficiency.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "2.6B",
        "updated": "September 23"
    },
    {
        "name": "LFM2-350M-Extract",
        "provider": "LiquidAI",
        "description": "Based on LFM2-350M. Finetuned for structured data extraction from unstructured text. Designed for fast, efficient on-device AI.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "350M",
        "updated": "September 10"
    },
    {
        "name": "LFM2-350M-ENJP-MT",
        "provider": "LiquidAI",
        "description": "Finetuned for near real-time Japanese/English translation of short-to-medium contexts.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "350M",
        "updated": "September 4"
    },
    {
        "name": "LFM2-VL-450M",
        "provider": "LiquidAI",
        "description": "First LFM2 multimodal series. Processes text and images at variable resolutions. Optimized for low-latency edge applications.",
        "modalities": ["text", "image"],
        "quantization": ["F16", "Q8_0"],
        "parameters": "450M",
        "updated": "August 27"
    },
    {
        "name": "LFM2-350M-Math",
        "provider": "LiquidAI",
        "description": "Finetuned LFM2-350M for mathematical problem solving. Designed for memory and compute efficiency.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "350M",
        "updated": "August 19"
    },
    {
        "name": "LFM2-1.2B-Tool",
        "provider": "LiquidAI",
        "description": "Finetuned for function-calling and agentic workflows. Memory and compute efficient for on-device gen-AI.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "August 19"
    },
    {
        "name": "LFM2-1.2B-Extract",
        "provider": "LiquidAI",
        "description": "Finetuned for structured data extraction from unstructured input. Designed for efficient on-device AI.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "August 19"
    },
    {
        "name": "LFM2-1.2B-RAG",
        "provider": "LiquidAI",
        "description": "Finetuned for retrieval-augmented generation (RAG) tasks. Efficient deployment on device.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "1.2B",
        "updated": "August 19"
    },
    {
        "name": "LFM2-700M",
        "provider": "LiquidAI",
        "description": "Hybrid-architecture model designed for fast, efficient on-device gen-AI workloads.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "700M",
        "updated": "July 11"
    },
    {
        "name": "LFM2-350M",
        "provider": "LiquidAI",
        "description": "Hybrid-architecture model designed for fast, efficient on-device gen-AI workloads.",
        "modalities": ["text"],
        "quantization": ["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"],
        "parameters": "350M",
        "updated": "July 11"
    }
]