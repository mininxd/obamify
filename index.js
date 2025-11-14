import init, { Generator } from './pkg/obamify.js';

class Obamify {
    constructor(options = {}) {
        this.width = options.width || 128;
        this.height = options.height || 128;
        this.resolution = options.resolution || 120;
        this.method = options.method || 'fast';
        this.gpu = options.gpu || false;
        this.onProgress = options.onProgress || (() => {});
        this.generator = null;

        if (this.gpu) {
            console.warn('GPU support is not yet implemented. Falling back to CPU.');
        }
    }

    async initialize() {
        await init();
        const settings = {
            sidelen: this.resolution,
            algorithm: this.method === 'optimal' ? 'Optimal' : 'Genetic',
            proximity_importance: 25,
            custom_target: null,
            target_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
            source_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
        };
        this.generator = new Generator(settings);
    }

    async generate(sourceImage, targetImage) {
        // GIF generation is not yet implemented.
        if (!this.generator) {
            await this.initialize();
        }

        return new Promise((resolve, reject) => {
            const worker = new Worker(new URL('./pkg/obamify.js', import.meta.url), { type: 'module' });

            worker.onmessage = (e) => {
                const msg = e.data;

                if (msg.typ === 'progress') {
                    this.onProgress(msg.payload);
                } else if (msg.typ === 'done') {
                    worker.terminate();
                    resolve(msg.payload);
                } else if (msg.typ === 'error') {
                    worker.terminate();
                    reject(msg.payload);
                } else {
                    // This is the final result
                    worker.terminate();
                    resolve(msg);
                }
            };

            worker.onerror = (e) => {
                worker.terminate();
                reject(e.message);
            };

            const sourcePreset = {
                name: 'source',
                width: this.width,
                height: this.height,
                source_img: Array.from(sourceImage),
            };

            const targetPreset = {
                name: 'target',
                width: this.width,
                height: this.height,
                source_img: Array.from(targetImage),
            };

            const settings = {
                sidelen: this.resolution,
                algorithm: this.method === 'optimal' ? 'Optimal' : 'Genetic',
                proximity_importance: 25,
                custom_target: null,
                target_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
                source_crop_scale: { x: 0.0, y: 0.0, scale: 1.0 },
            };

            worker.postMessage({
                type: 'Process',
                source: sourcePreset,
                target: targetPreset,
                settings,
            });
        });
    }
}

export default Obamify;
