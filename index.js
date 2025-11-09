import init, { Generator } from './pkg/obamify.js';

class Obamify {
    constructor(options = {}) {
        this.width = options.width || 128;
        this.height = options.height || 128;
        this.resolution = options.resolution || 120;
        this.method = options.method || 'fast';
        this.gpu = options.gpu || false;
        this.generator = null;
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
        if (!this.generator) {
            await this.initialize();
        }

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

        return this.generator.generate(sourcePreset, targetPreset);
    }
}

export default Obamify;
