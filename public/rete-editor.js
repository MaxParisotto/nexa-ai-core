class ReteEditor {
    constructor(container) {
        this.container = container;
        this.nodes = [];
        this.nextId = 1;
        this.connections = [];
        this.selectedNode = null;
        this.modelsFetched = new Set(); // Track which servers have had their models fetched
        this.modelCache = new Map(); // Cache for storing fetched models
        this.initializeEditor();
    }

    initializeEditor() {
        this.container.style.position = 'relative';
        this.container.style.overflow = 'hidden';
    }

    createNodeContent(type = 'llm') {
        const content = document.createElement('div');
        content.className = 'node-content';

        // Create a single container for all inputs
        const container = document.createElement('div');
        container.className = 'node-inputs-container';

        // Server selection
        const serverSelect = document.createElement('select');
        serverSelect.className = 'node-select';
        serverSelect.innerHTML = `
            <option value="">Select Server</option>
            <option value="http://localhost:1234/v1">LM Studio</option>
            <option value="http://localhost:11434">Ollama</option>
        `;

        // Model selection
        const modelSelectContainer = document.createElement('div');
        modelSelectContainer.className = 'select-wrapper';
        const modelSelect = document.createElement('select');
        modelSelect.className = 'node-select';
        modelSelect.innerHTML = '<option value="">Select Model</option>';
        modelSelect.disabled = true;

        // Loading indicator
        const loadingIndicator = document.createElement('div');
        loadingIndicator.className = 'loading-indicator';
        loadingIndicator.style.display = 'none';
        loadingIndicator.textContent = 'Loading models...';

        modelSelectContainer.appendChild(modelSelect);
        modelSelectContainer.appendChild(loadingIndicator);

        // Task input
        const taskInput = document.createElement('textarea');
        taskInput.className = 'node-input';
        taskInput.placeholder = 'Enter task description...';

        // Temperature control
        const tempContainer = document.createElement('div');
        tempContainer.className = 'param-row';
        const tempLabel = document.createElement('label');
        tempLabel.textContent = 'Temperature:';
        const tempSlider = document.createElement('input');
        tempSlider.type = 'range';
        tempSlider.className = 'param-slider';
        tempSlider.min = '0';
        tempSlider.max = '100';
        tempSlider.value = '70';
        const tempValue = document.createElement('span');
        tempValue.className = 'param-value';
        tempValue.textContent = '0.7';
        tempContainer.appendChild(tempLabel);
        tempContainer.appendChild(tempSlider);
        tempContainer.appendChild(tempValue);

        // Add elements to container
        container.appendChild(serverSelect);
        container.appendChild(modelSelectContainer);
        container.appendChild(taskInput);
        container.appendChild(tempContainer);

        // Add container to content
        content.appendChild(container);

        // Update models when server changes
        serverSelect.addEventListener('change', async (e) => {
            const serverUrl = e.target.value;
            if (!serverUrl) {
                modelSelect.innerHTML = '<option value="">Select Model</option>';
                modelSelect.disabled = true;
                return;
            }

            // If we have cached models, use them
            if (this.modelCache.has(serverUrl)) {
                const cachedModels = this.modelCache.get(serverUrl);
                this.updateModelSelect(modelSelect, cachedModels);
                return;
            }

            try {
                modelSelect.disabled = true;
                loadingIndicator.style.display = 'block';
                modelSelect.innerHTML = '<option value="">Loading models...</option>';

                const { invoke } = window.__TAURI__.tauri;
                const command = serverUrl.includes('11434') ? 'fetch_models_ollama' : 'fetch_models_lmstudio';
                
                const result = await invoke(command, {
                    url: serverUrl,
                    timeout: 5000
                });

                // Log the raw response for debugging
                console.log('Raw server response:', result);

                // Handle the nested Result structure from Rust
                if (result && result.Ok) {
                    const models = Array.isArray(result.Ok) ? result.Ok : 
                                 (result.Ok.Ok && Array.isArray(result.Ok.Ok)) ? result.Ok.Ok : [];
                    
                    if (models.length > 0) {
                        this.modelCache.set(serverUrl, models);
                        this.updateModelSelect(modelSelect, models);
                    } else {
                        throw new Error('No models found');
                    }
                } else {
                    // If result.Err exists, use that error message
                    const errorMessage = result && result.Err ? result.Err : 'Failed to load models';
                    throw new Error(errorMessage);
                }
            } catch (error) {
                console.error('Error fetching models:', error);
                modelSelect.innerHTML = `<option value="">Error: ${error.message || 'Failed to load models'}</option>`;
                modelSelect.disabled = true;
            } finally {
                loadingIndicator.style.display = 'none';
            }
        });

        // Add temperature slider event listener
        tempSlider.addEventListener('input', (e) => {
            tempValue.textContent = (e.target.value / 100).toFixed(2);
        });

        return content;
    }

    parseModelsResponse(result) {
        // Log the raw response for debugging
        console.log('Parsing models response:', result);

        // Handle null or undefined result
        if (!result) return [];

        try {
            // If result is a Result type with Ok variant
            if (result && typeof result === 'object' && 'Ok' in result) {
                const okResult = result.Ok;
                
                // Handle double-wrapped Result
                if (okResult && typeof okResult === 'object' && 'Ok' in okResult) {
                    return Array.isArray(okResult.Ok) ? okResult.Ok : [];
                }
                
                // Handle single-wrapped Result
                return Array.isArray(okResult) ? okResult : [];
            }
            
            // If result is already an array
            if (Array.isArray(result)) {
                return result;
            }

            console.warn('Unexpected response format:', result);
            return [];
        } catch (error) {
            console.error('Error parsing models response:', error);
            return [];
        }
    }

    updateModelSelect(modelSelect, models) {
        // Log the models being processed
        console.log('Updating model select with models:', models);

        if (!Array.isArray(models)) {
            console.error('Models is not an array:', models);
            modelSelect.innerHTML = '<option value="">Error loading models</option>';
            modelSelect.disabled = true;
            return;
        }

        const options = models.map(model => {
            const modelName = typeof model === 'string' ? model : 
                            (model && model.name) ? model.name : 
                            String(model);
            return `<option value="${modelName}">${modelName}</option>`;
        }).join('');

        modelSelect.innerHTML = '<option value="">Select Model</option>' + options;
        modelSelect.disabled = false;
    }

    createToolsSection() {
        const toolsContainer = document.createElement('div');
        toolsContainer.className = 'tools-container';
        toolsContainer.innerHTML = `
            <div class="tools-header">Available Tools</div>
            <div class="tools-list">
                <label><input type="checkbox" value="web_search"> Web Search</label>
                <label><input type="checkbox" value="code_analysis"> Code Analysis</label>
                <label><input type="checkbox" value="file_operations"> File Operations</label>
                <label><input type="checkbox" value="data_processing"> Data Processing</label>
            </div>
        `;
        return toolsContainer;
    }

    createParametersSection() {
        const paramsContainer = document.createElement('div');
        paramsContainer.className = 'params-container';
        paramsContainer.innerHTML = `
            <div class="params-header">Parameters</div>
            <div class="param-item">
                <label>Temperature:</label>
                <input type="range" min="0" max="100" value="70" class="param-slider">
                <span class="param-value">0.7</span>
            </div>
            <div class="param-item">
                <label>Max Tokens:</label>
                <input type="number" value="2048" min="1" max="8192" class="param-input">
            </div>
        `;

        // Add event listener for temperature slider
        const slider = paramsContainer.querySelector('.param-slider');
        const value = paramsContainer.querySelector('.param-value');
        slider.addEventListener('input', (e) => {
            value.textContent = (e.target.value / 100).toFixed(2);
        });

        return paramsContainer;
    }

    addNode(type = 'llm') {
        return new Promise((resolve, reject) => {
            try {
                const node = document.createElement('div');
                node.className = 'rete-node';
                node.id = `node-${this.nextId++}`;
                
                // Set position
                node.style.left = `${Math.random() * (this.container.clientWidth - 300)}px`;
                node.style.top = `${Math.random() * (this.container.clientHeight - 400)}px`;
                node.style.width = '300px';
                node.style.height = '400px';

                // Add header with controls
                const header = document.createElement('div');
                header.className = 'node-header';
                
                const titleSpan = document.createElement('span');
                titleSpan.textContent = `Node ${this.nextId - 1}`;
                
                const controls = document.createElement('div');
                controls.className = 'node-controls';
                controls.innerHTML = `
                    <button class="node-btn run-btn" title="Run Node">▶</button>
                    <button class="node-btn delete-btn" title="Delete Node">×</button>
                `;

                header.appendChild(titleSpan);
                header.appendChild(controls);
                node.appendChild(header);

                // Add content
                const content = this.createNodeContent(type);
                node.appendChild(content);

                // Add resize handle
                const resizeHandle = document.createElement('div');
                resizeHandle.className = 'resize-handle';
                node.appendChild(resizeHandle);

                // Add connection points
                const inputPoint = document.createElement('div');
                inputPoint.className = 'connection-point input-point';
                const outputPoint = document.createElement('div');
                outputPoint.className = 'connection-point output-point';

                node.appendChild(inputPoint);
                node.appendChild(outputPoint);

                // Setup event listeners
                controls.querySelector('.run-btn').addEventListener('click', () => this.runNode(node));
                controls.querySelector('.delete-btn').addEventListener('click', () => this.deleteNode(node));

                // Make node draggable
                this.makeDraggable(node);

                // Add resize handling
                this.makeResizable(node, resizeHandle);

                // Add connection handling
                this.setupConnectionPoints(node);

                this.container.appendChild(node);
                this.nodes.push(node);
                resolve(node);
            } catch (error) {
                reject(error);
            }
        });
    }

    setupConnectionPoints(node) {
        const inputPoint = node.querySelector('.input-point');
        const outputPoint = node.querySelector('.output-point');
        let isConnecting = false;
        let tempLine = null;
        let rafId = null;

        const createSVG = () => {
            const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
            svg.style.position = 'absolute';
            svg.style.top = '0';
            svg.style.left = '0';
            svg.style.width = '100%';
            svg.style.height = '100%';
            svg.style.pointerEvents = 'none';
            svg.style.willChange = 'transform';
            
            const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            path.setAttribute('stroke', '#646cff');
            path.setAttribute('stroke-width', '2');
            path.setAttribute('fill', 'none');
            
            svg.appendChild(path);
            return svg;
        };

        const updateLine = (e, startX, startY) => {
            if (!isConnecting || !tempLine) return;
            
            if (rafId) {
                cancelAnimationFrame(rafId);
            }

            rafId = requestAnimationFrame(() => {
                const endX = e.clientX - this.container.getBoundingClientRect().left;
                const endY = e.clientY - this.container.getBoundingClientRect().top;
                const dx = endX - startX;
                const dy = endY - startY;
                
                const path = tempLine.querySelector('path');
                path.setAttribute('d', `M ${startX} ${startY} C ${startX + dx/2} ${startY}, ${startX + dx/2} ${endY}, ${endX} ${endY}`);
                
                rafId = null;
            });
        };

        outputPoint.addEventListener('mousedown', (e) => {
            isConnecting = true;
            const rect = outputPoint.getBoundingClientRect();
            const startX = rect.left + rect.width / 2 - this.container.getBoundingClientRect().left;
            const startY = rect.top + rect.height / 2 - this.container.getBoundingClientRect().top;

            tempLine = createSVG();
            this.container.appendChild(tempLine);

            const moveHandler = (e) => updateLine(e, startX, startY);
            document.addEventListener('mousemove', moveHandler, { passive: true });

            const cleanup = () => {
                document.removeEventListener('mousemove', moveHandler);
                if (tempLine) {
                    tempLine.remove();
                }
                if (rafId) {
                    cancelAnimationFrame(rafId);
                    rafId = null;
                }
                isConnecting = false;
                tempLine = null;
            };

            document.addEventListener('mouseup', cleanup, { once: true });
        });

        inputPoint.addEventListener('mouseup', (e) => {
            if (isConnecting && tempLine) {
                // Create permanent connection
                const connection = {
                    from: tempLine.dataset.fromNode,
                    to: node.id
                };
                this.connections.push(connection);
                this.drawConnections();
            }
        });
    }

    drawConnections() {
        // Remove existing connection lines
        const existingLines = this.container.querySelectorAll('.connection-line');
        existingLines.forEach(line => line.remove());

        // Create a single SVG for all connections
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        svg.style.position = 'absolute';
        svg.style.top = '0';
        svg.style.left = '0';
        svg.style.width = '100%';
        svg.style.height = '100%';
        svg.style.pointerEvents = 'none';
        svg.classList.add('connection-line');

        // Draw all connections in a single batch
        this.connections.forEach(conn => {
            const fromNode = this.container.querySelector(`#${conn.from}`);
            const toNode = this.container.querySelector(`#${conn.to}`);
            
            if (fromNode && toNode) {
                const fromPoint = fromNode.querySelector('.output-point');
                const toPoint = toNode.querySelector('.input-point');
                
                const fromRect = fromPoint.getBoundingClientRect();
                const toRect = toPoint.getBoundingClientRect();
                const containerRect = this.container.getBoundingClientRect();
                
                const startX = fromRect.left + fromRect.width / 2 - containerRect.left;
                const startY = fromRect.top + fromRect.height / 2 - containerRect.top;
                const endX = toRect.left + toRect.width / 2 - containerRect.left;
                const endY = toRect.top + toRect.height / 2 - containerRect.top;
                
                const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
                path.setAttribute('stroke', '#646cff');
                path.setAttribute('stroke-width', '2');
                path.setAttribute('fill', 'none');
                path.setAttribute('d', `M ${startX} ${startY} C ${startX} ${startY + 50}, ${endX} ${endY - 50}, ${endX} ${endY}`);
                
                svg.appendChild(path);
            }
        });

        this.container.appendChild(svg);
    }

    runNode(node) {
        const serverSelect = node.querySelector('.server-select');
        const modelSelect = node.querySelector('.model-select');
        const taskInput = node.querySelector('.task-input');
        const tools = Array.from(node.querySelectorAll('.tools-list input:checked')).map(cb => cb.value);
        const temperature = node.querySelector('.param-slider').value / 100;
        const maxTokens = node.querySelector('.param-input').value;

        const config = {
            server: serverSelect.value,
            model: modelSelect.value,
            task: taskInput.value,
            tools,
            parameters: {
                temperature,
                maxTokens: parseInt(maxTokens)
            }
        };

        console.log('Running node with config:', config);
        // Here you would implement the actual LLM call
    }

    deleteNode(node) {
        // Remove connections involving this node
        this.connections = this.connections.filter(conn => 
            conn.from !== node.id && conn.to !== node.id);
        
        // Remove the node
        node.remove();
        this.nodes = this.nodes.filter(n => n !== node);
        
        // Redraw remaining connections
        this.drawConnections();
    }

    makeDraggable(node) {
        let isDragging = false;
        let currentX;
        let currentY;
        let initialX;
        let initialY;
        let rafId = null;

        node.addEventListener('mousedown', (e) => {
            if (e.target.closest('.node-controls, .node-select, .node-input, .tools-list, .params-container')) {
                return;
            }
            isDragging = true;
            node.classList.add('dragging');
            
            initialX = e.clientX - node.offsetLeft;
            initialY = e.clientY - node.offsetTop;
            
            // Force hardware acceleration
            node.style.willChange = 'transform';
        });

        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;
            e.preventDefault();
            
            // Cancel any existing animation frame
            if (rafId) {
                cancelAnimationFrame(rafId);
            }

            // Schedule the update
            rafId = requestAnimationFrame(() => {
                currentX = e.clientX - initialX;
                currentY = e.clientY - initialY;

                // Boundary checks
                currentX = Math.max(0, Math.min(currentX, this.container.clientWidth - node.offsetWidth));
                currentY = Math.max(0, Math.min(currentY, this.container.clientHeight - node.offsetHeight));

                // Use transform instead of left/top for better performance
                node.style.transform = `translate(${currentX}px, ${currentY}px)`;
                
                rafId = null;
            });
        }, { passive: true });

        document.addEventListener('mouseup', () => {
            if (!isDragging) return;
            
            isDragging = false;
            node.classList.remove('dragging');
            node.style.willChange = 'auto';
            
            // Convert transform to left/top for final position
            const transform = new WebKitCSSMatrix(window.getComputedStyle(node).transform);
            node.style.transform = 'none';
            node.style.left = `${transform.m41}px`;
            node.style.top = `${transform.m42}px`;
            
            if (rafId) {
                cancelAnimationFrame(rafId);
                rafId = null;
            }
        });
    }

    makeResizable(node, handle) {
        let isResizing = false;
        let startWidth, startHeight, startX, startY;
        let rafId = null;

        const startResize = (e) => {
            if (e.button !== 0) return;
            isResizing = true;
            handle.classList.add('resizing');
            
            startWidth = node.offsetWidth;
            startHeight = node.offsetHeight;
            startX = e.clientX;
            startY = e.clientY;
            
            // Force hardware acceleration
            node.style.willChange = 'width, height';
            
            e.preventDefault();
            e.stopPropagation();
        };

        const doResize = (e) => {
            if (!isResizing) return;
            e.preventDefault();

            // Cancel any existing animation frame
            if (rafId) {
                cancelAnimationFrame(rafId);
            }

            // Schedule the update
            rafId = requestAnimationFrame(() => {
                const dx = e.clientX - startX;
                const dy = e.clientY - startY;

                const newWidth = Math.max(300, Math.min(800, startWidth + dx));
                const newHeight = Math.max(300, Math.min(800, startHeight + dy));

                node.style.width = `${newWidth}px`;
                node.style.height = `${newHeight}px`;
                
                rafId = null;
            });
        };

        const stopResize = () => {
            if (!isResizing) return;
            isResizing = false;
            handle.classList.remove('resizing');
            node.style.willChange = 'auto';
            
            if (rafId) {
                cancelAnimationFrame(rafId);
                rafId = null;
            }
        };

        handle.addEventListener('mousedown', startResize);
        document.addEventListener('mousemove', doResize, { passive: true });
        document.addEventListener('mouseup', stopResize);
    }

    clear() {
        this.nodes.forEach(node => node.remove());
        this.nodes = [];
        this.nextId = 1;
        this.connections = [];
        this.container.querySelectorAll('.connection-line').forEach(line => line.remove());
    }

    destroy() {
        this.clear();
        this.container.innerHTML = '';
    }
}

// Make ReteEditor available globally
window.ReteEditor = ReteEditor; 