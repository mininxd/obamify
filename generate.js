import init, { worker_entry } from './obamify.js';

class Generator {
    constructor(options = {}) {
        this.proximity = options.proximity || 25;
        this.method = options.method || 'optimal'; // or 'fast'
        this.resolution = options.resolution || 128;
        this.onProgress = options.onProgress || (() => {});
        this.wasm_initialized = false;
    }

    async init_wasm() {
        if (!this.wasm_initialized) {
            await init('./obamify_bg.wasm');
            this.wasm_initialized = true;
        }
    }

    async generate(sourceImage, targetImage) {
        await this.init_wasm();

        return new Promise((resolve, reject) => {
            const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });

            worker.onmessage = (e) => {
                const msg = e.data;
                if (msg.type === 'Progress') {
                    this.onProgress(msg.payload);
                } else if (msg.type === 'Done') {
                    worker.terminate();
                    resolve(msg.payload);
                } else if (msg.type === 'Error') {
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
                width: sourceImage.width,
                height: sourceImage.height,
                source_img: Array.from(sourceImage.data),
            };

            const targetPreset = {
                name: 'target',
                width: targetImage.width,
                height: targetImage.height,
                source_img: Array.from(targetImage.data),
            }

            const settings = {
                id,
                name: 'obamify',
                proximity_importance: this.proximity,
                algorithm: this.method === 'optimal' ? 'Optimal' : 'Genetic',
                sidelen: this.resolution,
                custom_target: {
                    w: targetImage.width,
                    h: targetImage.height,
                    data: Array.from(targetImage.data),
                },
                target_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
                source_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
            };

            const req = {
                Process: {
                    source: sourcePreset,
                    target: targetPreset,
                    settings,
                },
            };

            worker.postMessage(req);
        });
    }
}

export default Generator;
