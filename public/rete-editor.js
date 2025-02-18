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

        switch (type) {
            case 'llm':
                // Server selection
                const serverSelect = document.createElement('select');
                serverSelect.className = 'node-select server-select';
                serverSelect.innerHTML = `
                    <option value="">Select Server</option>
                    <option value="http://localhost:1234/v1">LM Studio</option>
                    <option value="http://localhost:11434">Ollama</option>
                `;

                // Model selection container
                const modelSelectContainer = document.createElement('div');
                modelSelectContainer.className = 'node-select-container';

                // Model selection
                const modelSelect = document.createElement('select');
                modelSelect.className = 'node-select model-select';
                modelSelect.innerHTML = '<option value="">Select Model</option>';
                modelSelect.disabled = true;

                modelSelectContainer.appendChild(modelSelect);

                // Loading indicator
                const loadingIndicator = document.createElement('div');
                loadingIndicator.className = 'loading-indicator';
                loadingIndicator.style.display = 'none';
                loadingIndicator.innerHTML = 'Loading models...';

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
                        console.log('Using cached models for', serverUrl);
                        const cachedModels = this.modelCache.get(serverUrl);
                        modelSelect.innerHTML = '<option value="">Select Model</option>' +
                            cachedModels.map(model => `<option value="${model}">${model}</option>`).join('');
                        modelSelect.disabled = false;
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

                        console.log('Server response:', result);

                        let models = [];
                        if (result && Array.isArray(result)) {
                            models = result;
                        } else if (result && typeof result === 'object') {
                            if (result.Ok) {
                                const innerResult = result.Ok;
                                if (Array.isArray(innerResult)) {
                                    models = innerResult;
                                } else if (innerResult && innerResult.Ok && Array.isArray(innerResult.Ok)) {
                                    models = innerResult.Ok;
                                }
                            }
                        }

                        if (models.length > 0) {
                            // Cache the models
                            this.modelCache.set(serverUrl, models);
                            
                            // Update the select
                            modelSelect.innerHTML = '<option value="">Select Model</option>' +
                                models.map(model => `<option value="${model}">${model}</option>`).join('');
                            modelSelect.disabled = false;
                        } else {
                            throw new Error('No models found');
                        }
                    } catch (error) {
                        console.error('Error fetching models:', error);
                        modelSelect.innerHTML = `<option value="">Error: ${error.message || 'Failed to load models'}</option>`;
                        modelSelect.disabled = true;
                    } finally {
                        loadingIndicator.style.display = 'none';
                    }
                });

                // Task input
                const taskInput = document.createElement('textarea');
                taskInput.className = 'node-input task-input';
                taskInput.placeholder = 'Enter task description...';

                // Tools section
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

                // Parameters section
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

                // Add all elements to content
                content.appendChild(serverSelect);
                content.appendChild(modelSelectContainer);
                content.appendChild(loadingIndicator);
                content.appendChild(taskInput);
                content.appendChild(toolsContainer);
                content.appendChild(paramsContainer);
                break;

            // Add more node types here if needed
        }

        return content;
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

        outputPoint.addEventListener('mousedown', (e) => {
            isConnecting = true;
            const rect = outputPoint.getBoundingClientRect();
            const startX = rect.left + rect.width / 2 - this.container.getBoundingClientRect().left;
            const startY = rect.top + rect.height / 2 - this.container.getBoundingClientRect().top;

            tempLine = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
            tempLine.style.position = 'absolute';
            tempLine.style.top = '0';
            tempLine.style.left = '0';
            tempLine.style.width = '100%';
            tempLine.style.height = '100%';
            tempLine.style.pointerEvents = 'none';
            
            const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            path.setAttribute('stroke', '#646cff');
            path.setAttribute('stroke-width', '2');
            path.setAttribute('fill', 'none');
            
            tempLine.appendChild(path);
            this.container.appendChild(tempLine);

            const updateLine = (e) => {
                if (!isConnecting) return;
                const endX = e.clientX - this.container.getBoundingClientRect().left;
                const endY = e.clientY - this.container.getBoundingClientRect().top;
                const dx = endX - startX;
                const dy = endY - startY;
                const path = tempLine.querySelector('path');
                path.setAttribute('d', `M ${startX} ${startY} C ${startX + dx/2} ${startY}, ${startX + dx/2} ${endY}, ${endX} ${endY}`);
            };

            document.addEventListener('mousemove', updateLine);
            document.addEventListener('mouseup', () => {
                document.removeEventListener('mousemove', updateLine);
                if (tempLine) {
                    tempLine.remove();
                }
                isConnecting = false;
            }, { once: true });
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
        this.container.querySelectorAll('.connection-line').forEach(line => line.remove());

        // Redraw all connections
        this.connections.forEach(conn => {
            const fromNode = this.container.querySelector(`#${conn.from}`);
            const toNode = this.container.querySelector(`#${conn.to}`);
            if (fromNode && toNode) {
                const fromPoint = fromNode.querySelector('.output-point');
                const toPoint = toNode.querySelector('.input-point');
                // Create and add connection line
                // ... (implementation similar to temporary line creation)
            }
        });
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

        node.addEventListener('mousedown', (e) => {
            if (e.target.closest('.node-controls, .node-select, .node-input, .tools-list, .params-container')) {
                return; // Don't initiate drag on controls
            }
            isDragging = true;
            node.classList.add('dragging');
            
            initialX = e.clientX - node.offsetLeft;
            initialY = e.clientY - node.offsetTop;
        });

        document.addEventListener('mousemove', (e) => {
            if (isDragging) {
                e.preventDefault();
                
                currentX = e.clientX - initialX;
                currentY = e.clientY - initialY;

                // Boundary checks
                currentX = Math.max(0, Math.min(currentX, this.container.clientWidth - node.offsetWidth));
                currentY = Math.max(0, Math.min(currentY, this.container.clientHeight - node.offsetHeight));

                node.style.left = `${currentX}px`;
                node.style.top = `${currentY}px`;
                
                // Update connections
                this.drawConnections();
            }
        });

        document.addEventListener('mouseup', () => {
            isDragging = false;
            node.classList.remove('dragging');
        });
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