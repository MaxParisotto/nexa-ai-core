class ReteEditor {
    constructor(container) {
        this.container = container;
        this.nodes = [];
        this.nextId = 1;
        this.initializeEditor();
    }

    initializeEditor() {
        // Add any necessary styles
        this.container.style.position = 'relative';
        this.container.style.overflow = 'hidden';
        this.container.style.backgroundColor = '#f5f5f5';
    }

    addNode() {
        return new Promise((resolve, reject) => {
            try {
                const node = document.createElement('div');
                node.className = 'rete-node';
                node.id = `node-${this.nextId++}`;
                node.style.cssText = `
                    position: absolute;
                    width: 200px;
                    min-height: 100px;
                    background: white;
                    border: 1px solid #ccc;
                    border-radius: 8px;
                    padding: 10px;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                    cursor: move;
                    left: ${Math.random() * (this.container.clientWidth - 200)}px;
                    top: ${Math.random() * (this.container.clientHeight - 100)}px;
                `;

                // Add header
                const header = document.createElement('div');
                header.className = 'node-header';
                header.style.cssText = `
                    padding: 5px;
                    background: #f0f0f0;
                    border-radius: 6px 6px 0 0;
                    margin-bottom: 10px;
                    font-weight: bold;
                `;
                header.textContent = `Node ${this.nextId - 1}`;
                node.appendChild(header);

                // Make node draggable
                this.makeDraggable(node);

                this.container.appendChild(node);
                this.nodes.push(node);
                resolve();
            } catch (error) {
                reject(error);
            }
        });
    }

    makeDraggable(node) {
        let isDragging = false;
        let currentX;
        let currentY;
        let initialX;
        let initialY;

        node.addEventListener('mousedown', (e) => {
            isDragging = true;
            node.style.cursor = 'grabbing';
            
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
            }
        });

        document.addEventListener('mouseup', () => {
            isDragging = false;
            node.style.cursor = 'move';
        });
    }

    clear() {
        this.nodes.forEach(node => node.remove());
        this.nodes = [];
        this.nextId = 1;
    }

    destroy() {
        this.clear();
        this.container.innerHTML = '';
    }
}

// Make ReteEditor available globally
window.ReteEditor = ReteEditor; 