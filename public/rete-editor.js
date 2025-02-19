class ReteEditor {
    constructor(container) {
        this.container = container;
        this.nodes = [];
        this.nextId = 1;
        this.connections = [];
        this.selectedNode = null;
        this.modelsFetched = new Set(); // Track which servers have had their models fetched
        this.modelCache = new Map(); // Cache for storing fetched models
        this.activeConnections = new Map(); // Track active server connections
        this.outputBuffers = new Map(); // Buffer for node outputs
        this.taskRoutes = new Map(); // Route configuration for tasks
        this.bufferLogs = new Map(); // Store buffer logs for each node
        
        this.initializeEditor();
        this.createBufferLogPanel();
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

            await this.fetchModels(serverUrl, modelSelect, loadingIndicator);
        });

        // Add temperature slider event listener
        tempSlider.addEventListener('input', (e) => {
            tempValue.textContent = (e.target.value / 100).toFixed(2);
        });

        return content;
    }

    async fetchModels(serverUrl, modelSelect, loadingIndicator) {
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

            console.log('Raw server response:', result);

            let models = [];
            
            if (result && typeof result === 'object') {
                // Handle the outer Result
                if ('Ok' in result) {
                    const innerResult = result.Ok;
                    // Handle the inner Result
                    if (innerResult && typeof innerResult === 'object') {
                        if ('Ok' in innerResult) {
                            models = innerResult.Ok;
                        } else if ('Err' in innerResult) {
                            throw new Error(innerResult.Err);
                        }
                    }
                } else if ('Err' in result) {
                    throw new Error(result.Err);
                }
            }

            if (!Array.isArray(models) || models.length === 0) {
                throw new Error('No models found');
            }

            this.modelCache.set(serverUrl, models);
            this.updateModelSelect(modelSelect, models);

        } catch (error) {
            console.error('Error fetching models:', error);
            modelSelect.innerHTML = `<option value="">Error: ${error.message || 'Failed to load models'}</option>`;
            modelSelect.disabled = true;
        } finally {
            loadingIndicator.style.display = 'none';
        }
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
        console.log('Updating model select with models:', models);

        if (!Array.isArray(models) || models.length === 0) {
            console.error('Invalid models array:', models);
            modelSelect.innerHTML = '<option value="">No models available</option>';
            modelSelect.disabled = true;
            return;
        }

        try {
            const options = models.map(model => {
                const modelName = typeof model === 'string' ? model : 
                                (model && typeof model.name) ? model.name :
                                String(model);
                return `<option value="${modelName}">${modelName}</option>`;
            }).join('');

            modelSelect.innerHTML = '<option value="">Select Model</option>' + options;
            modelSelect.disabled = false;
        } catch (error) {
            console.error('Error creating model options:', error);
            modelSelect.innerHTML = '<option value="">Error loading models</option>';
            modelSelect.disabled = true;
        }
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

    async runNode(node) {
        try {
            const serverSelect = node.querySelector('.node-select');
            const modelSelect = node.querySelector('.node-select');
            const taskInput = node.querySelector('.node-input');
            const tempSlider = node.querySelector('.param-slider');
            
            if (!serverSelect.value || !modelSelect.value || !taskInput.value.trim()) {
                throw new Error('Please fill in all required fields');
            }

            // Update node status
            node.classList.add('processing');
            const statusIndicator = document.createElement('div');
            statusIndicator.className = 'node-status';
            statusIndicator.textContent = 'Processing...';
            node.appendChild(statusIndicator);

            // Prepare the chat request
            const message = taskInput.value.trim();
            const temperature = parseFloat(tempSlider.value) / 100;

            // Get connected output nodes
            const outputNodes = this.getConnectedOutputNodes(node.id);
            
            // Create output buffer for this node if it doesn't exist
            if (!this.outputBuffers.has(node.id)) {
                this.outputBuffers.set(node.id, []);
            }

            const { invoke } = window.__TAURI__.tauri;
            const result = await invoke('chat_completion', {
                server_url: serverSelect.value,
                model: modelSelect.value,
                message: message,
                temperature: temperature
            });

            // Process the result
            if (result.Ok) {
                const response = result.Ok;
                
                // Buffer the output
                this.outputBuffers.get(node.id).push({
                    timestamp: new Date().toISOString(),
                    input: message,
                    output: response,
                    metadata: {
                        model: modelSelect.value,
                        temperature: temperature
                    }
                });

                // Route the output to connected nodes
                for (const outputNode of outputNodes) {
                    await this.routeOutput(node.id, outputNode, response);
                }

                // Update node status
                statusIndicator.textContent = 'Completed';
                statusIndicator.classList.add('success');
            } else if (result.Err) {
                throw new Error(result.Err);
            }

        } catch (error) {
            console.error('Node execution error:', error);
            const statusIndicator = node.querySelector('.node-status');
            if (statusIndicator) {
                statusIndicator.textContent = `Error: ${error.message}`;
                statusIndicator.classList.add('error');
            }
        } finally {
            node.classList.remove('processing');
            // Remove status indicator after delay
            setTimeout(() => {
                const statusIndicator = node.querySelector('.node-status');
                if (statusIndicator) {
                    statusIndicator.remove();
                }
            }, 3000);
        }
    }

    getConnectedOutputNodes(nodeId) {
        return this.connections
            .filter(conn => conn.from === nodeId)
            .map(conn => this.nodes.find(n => n.id === conn.to))
            .filter(Boolean);
    }

    async routeOutput(sourceNodeId, targetNode, output) {
        const targetInput = targetNode.querySelector('.node-input');
        if (targetInput) {
            // Get the route configuration for this connection
            const routeConfig = this.taskRoutes.get(`${sourceNodeId}-${targetNode.id}`);
            
            // Apply routing logic based on configuration
            let processedOutput = output;
            if (routeConfig && routeConfig.transform) {
                processedOutput = await routeConfig.transform(output);
            }

            // Update the target node's input
            targetInput.value = processedOutput;
            
            // Trigger any necessary updates
            const event = new Event('input', { bubbles: true });
            targetInput.dispatchEvent(event);
        }
    }

    // Add routing configuration for a connection
    setTaskRoute(sourceNodeId, targetNodeId, config) {
        this.taskRoutes.set(`${sourceNodeId}-${targetNodeId}`, config);
    }

    // Get buffered outputs for a node
    getNodeOutputs(nodeId) {
        return this.outputBuffers.get(nodeId) || [];
    }

    // Clear output buffer for a node
    clearNodeOutputs(nodeId) {
        this.outputBuffers.delete(nodeId);
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

    // Add this new method to create the buffer log panel
    createBufferLogPanel() {
        // Create the buffer log button in the status bar
        const statusActions = document.querySelector('.status-actions');
        if (!statusActions) return;

        const bufferLogBtn = document.createElement('button');
        bufferLogBtn.className = 'buffer-logs-btn';
        bufferLogBtn.title = 'Buffer Logs';
        bufferLogBtn.innerHTML = `
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 12V7H3v5"/>
                <path d="M3 17h18"/>
                <path d="M21 7v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
            </svg>
        `;

        // Create the buffer log panel
        const logPanel = document.createElement('div');
        logPanel.className = 'modal buffer-logs-modal';
        logPanel.style.display = 'none';
        logPanel.innerHTML = `
            <div class="modal-content buffer-logs-content">
                <div class="modal-header">
                    <h2>Buffer Logs</h2>
                    <div class="logs-actions">
                        <button class="clear-btn" title="Clear Logs">
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M3 6h18"/>
                                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                            </svg>
                        </button>
                        <button class="close-btn">
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <line x1="18" y1="6" x2="6" y2="18"/>
                                <line x1="6" y1="6" x2="18" y2="18"/>
                            </svg>
                        </button>
                    </div>
                </div>
                <div class="buffer-log-content">
                    <div class="buffer-log-entries"></div>
                </div>
                <div class="resize-handle"></div>
            </div>
        `;

        // Add event listeners
        bufferLogBtn.addEventListener('click', () => {
            logPanel.style.display = logPanel.style.display === 'none' ? 'block' : 'none';
            bufferLogBtn.classList.toggle('active');
        });

        const closeBtn = logPanel.querySelector('.close-btn');
        closeBtn.addEventListener('click', () => {
            logPanel.style.display = 'none';
            bufferLogBtn.classList.remove('active');
        });

        const clearBtn = logPanel.querySelector('.clear-btn');
        clearBtn.addEventListener('click', () => {
            this.clearBufferLogs();
        });

        // Make the panel draggable and resizable
        this.makeModalDraggable(logPanel);
        this.makeModalResizable(logPanel);

        // Add the button and panel to the DOM
        statusActions.appendChild(bufferLogBtn);
        document.body.appendChild(logPanel);
        this.bufferLogPanel = logPanel;
    }

    // Add helper method for making modals draggable
    makeModalDraggable(modal) {
        const header = modal.querySelector('.modal-header');
        let isDragging = false;
        let currentX;
        let currentY;
        let initialX;
        let initialY;

        header.addEventListener('mousedown', (e) => {
            if (e.target.closest('.logs-actions')) return;
            isDragging = true;
            initialX = e.clientX - modal.offsetLeft;
            initialY = e.clientY - modal.offsetTop;
        });

        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;
            e.preventDefault();
            
            currentX = e.clientX - initialX;
            currentY = e.clientY - initialY;
            
            modal.style.left = `${currentX}px`;
            modal.style.top = `${currentY}px`;
        });

        document.addEventListener('mouseup', () => {
            isDragging = false;
        });
    }

    // Add helper method for making modals resizable
    makeModalResizable(modal) {
        const handle = modal.querySelector('.resize-handle');
        const content = modal.querySelector('.modal-content');
        let isResizing = false;
        let startWidth;
        let startHeight;
        let startX;
        let startY;

        handle.addEventListener('mousedown', (e) => {
            isResizing = true;
            startWidth = content.offsetWidth;
            startHeight = content.offsetHeight;
            startX = e.clientX;
            startY = e.clientY;
            e.preventDefault();
        });

        document.addEventListener('mousemove', (e) => {
            if (!isResizing) return;
            
            const dx = e.clientX - startX;
            const dy = e.clientY - startY;
            
            content.style.width = `${Math.max(400, startWidth + dx)}px`;
            content.style.height = `${Math.max(300, startHeight + dy)}px`;
        });

        document.addEventListener('mouseup', () => {
            isResizing = false;
        });
    }

    // Add this method to log buffer operations
    logBufferOperation(nodeId, type, data) {
        if (!this.bufferLogs.has(nodeId)) {
            this.bufferLogs.set(nodeId, []);
        }

        const logEntry = {
            timestamp: new Date().toISOString(),
            type,
            data,
        };

        this.bufferLogs.get(nodeId).push(logEntry);
        this.updateBufferLogDisplay(nodeId, logEntry);
    }

    // Add this method to update the log display
    updateBufferLogDisplay(nodeId, logEntry) {
        const logEntries = this.bufferLogPanel.querySelector('.buffer-log-entries');
        const entry = document.createElement('div');
        entry.className = `buffer-log-entry ${logEntry.type}`;
        
        const timestamp = new Date(logEntry.timestamp).toLocaleTimeString();
        const nodeName = this.nodes.find(n => n.id === nodeId)?.querySelector('.node-header span')?.textContent || nodeId;

        entry.innerHTML = `
            <div class="timestamp">${timestamp}</div>
            <div class="node-info">${nodeName}</div>
            <div class="data-flow">
                ${this.formatLogData(logEntry.type, logEntry.data)}
            </div>
        `;

        logEntries.appendChild(entry);
        logEntries.scrollTop = logEntries.scrollHeight;
    }

    // Add this method to format log data
    formatLogData(type, data) {
        switch (type) {
            case 'input':
                return `📥 Input: ${this.truncateText(data.input)}`;
            case 'output':
                return `📤 Output: ${this.truncateText(data.output)}`;
            case 'error':
                return `❌ Error: ${data.message}`;
            default:
                return JSON.stringify(data);
        }
    }

    // Add this helper method
    truncateText(text, length = 100) {
        return text.length > length ? text.substring(0, length) + '...' : text;
    }

    // Add this method to clear logs
    clearBufferLogs() {
        this.bufferLogs.clear();
        const logEntries = this.bufferLogPanel.querySelector('.buffer-log-entries');
        logEntries.innerHTML = '';
    }
}

// Make ReteEditor available globally
window.ReteEditor = ReteEditor; 