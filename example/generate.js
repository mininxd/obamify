import init, { generate_with_gpu } from './pkg/obamify.js';

class Generator {
    constructor(options = {}) {
        this.width = options.width || 128;
        this.height = options.height || 128;
        this.gpu = options.gpu || false;
        this.proximity = options.proximity || 25;
        this.method = options.method || 'optimal'; // or 'fast'
        this.resolution = options.resolution || 128;
        this.onProgress = options.onProgress || (() => {});
        this.wasm_initialized = false;
    }

    async init_wasm() {
        if (!this.wasm_initialized) {
            await init('./pkg/obamify_bg.wasm');
            this.wasm_initialized = true;
        }
    }

    async generate(sourceImageData, targetImageData) {
        await this.init_wasm();

        return new Promise((resolve, reject) => {
            const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });

            worker.onmessage = (e) => {
                const msg = e.data;
                if (msg.type === 'progress') {
                    this.onProgress(msg.payload);
                } else if (msg.type === 'done') {
                    worker.terminate();
                    resolve(msg.payload);
                } else if (msg.type === 'error') {
                    worker.terminate();
                    reject(msg.payload);
                }
            };

            worker.onerror = (e) => {
                worker.terminate();
                reject(e.message);
            };

            const id = '00000000-0000-0000-0000-000000000000'; // Placeholder UUID

            const sourcePreset = {
                name: 'source',
                width: this.width,
                height: this.height,
                source_img: Array.from(sourceImageData),
            };

            const targetPreset = {
                name: 'target',
                width: this.width,
                height: this.height,
                source_img: Array.from(targetImageData),
            }

            const settings = {
                id,
                name: 'obamify',
                proximity_importance: this.proximity,
                algorithm: this.method === 'optimal' ? 'Optimal' : 'Genetic',
                sidelen: this.resolution,
                custom_target: {
                    w: this.width,
                    h: this.height,
                    data: Array.from(targetImageData),
                },
                target_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
                source_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
            };

            worker.postMessage({
                source: sourcePreset,
                target: targetPreset,
                settings,
                gpu: this.gpu,
            });
        });
    }
}

export default Generator;
