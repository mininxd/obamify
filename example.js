// To run this example, you would first need to publish the obamify package to npm.
// Then, you would install it in your project with `npm install obamify`.

import Generator from 'obamify';

async function main() {
    const generator = new Generator({
        gpu: true, // Use WebGPU for the computation
        onProgress: (progress) => {
            console.log(`Progress: ${progress * 100}%`);
        },
    });

    // Create placeholder images (replace with actual image loading)
    const sourceImage = {
        width: 128,
        height: 128,
        data: new Uint8Array(128 * 128 * 4).fill(255), // White image
    };

    const targetImage = {
        width: 128,
        height: 128,
        data: new Uint8Array(128 * 128 * 4).fill(0), // Black image
    };

    try {
        const result = await generator.generate(sourceImage, targetImage);
        console.log('Generation complete!');
        // Display the result (e.g., on a canvas)
        const canvas = document.createElement('canvas');
        canvas.width = generator.resolution;
        canvas.height = generator.resolution;
        const ctx = canvas.getContext('2d');
        const imageData = new ImageData(new Uint8ClampedArray(result), generator.resolution, generator.resolution);
        ctx.putImageData(imageData, 0, 0);
        document.body.appendChild(canvas);
    } catch (error) {
        console.error('Generation failed:', error);
    }
}

main();
