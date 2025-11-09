import init, { generate_with_gpu } from './pkg/obamify.js';

async function main() {
    await init('./pkg/obamify_bg.wasm');

    self.onmessage = async (e) => {
        const { source, target, settings, gpu } = e.data;
        try {
            const result = await generate_with_gpu(source, target, settings, gpu);
            self.postMessage({ type: 'done', payload: result });
        } catch (error) {
            self.postMessage({ type: 'error', payload: error });
        }
    };
}

main();
