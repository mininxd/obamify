import Generator from './generate.js';

async function loadImageData(file) {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = (e) => {
            const img = new Image();
            img.onload = () => {
                const canvas = document.createElement('canvas');
                canvas.width = img.width;
                canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0);
                resolve(ctx.getImageData(0, 0, img.width, img.height));
            };
            img.onerror = reject;
            img.src = e.target.result;
        };
        reader.onerror = reject;
        reader.readAsDataURL(file);
    });
}

const sourceImageInput = document.getElementById('sourceImage');
const targetImageInput = document.getElementById('targetImage');
const generateButton = document.getElementById('generateButton');
const outputCanvas = document.getElementById('outputCanvas');

let sourceImageData;
let targetImageData;

sourceImageInput.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (file) {
        sourceImageData = await loadImageData(file);
    }
});

targetImageInput.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (file) {
        targetImageData = await loadImageData(file);
    }
});

generateButton.addEventListener('click', async () => {
    if (!sourceImageData || !targetImageData) {
        alert('Please select both a source and a target image.');
        return;
    }

    const generator = new Generator({
        width: sourceImageData.width,
        height: sourceImageData.height,
        gpu: true,
        onProgress: (progress) => {
            console.log(`Progress: ${progress * 100}%`);
        },
    });

    try {
        const result = await generator.generate(sourceImageData.data, targetImageData.data);
        console.log('Generation complete!');
        outputCanvas.width = generator.resolution;
        outputCanvas.height = generator.resolution;
        const ctx = outputCanvas.getContext('2d');
        const imageData = new ImageData(new Uint8ClampedArray(result), generator.resolution, generator.resolution);
        ctx.putImageData(imageData, 0, 0);
    } catch (error) {
        console.error('Generation failed:', error);
    }
});
