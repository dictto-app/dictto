/**
 * Dictto Icon Generation Script
 *
 * Converts SVG source logos into all PNG/ICO assets required by Tauri v2.
 * Uses @resvg/resvg-js for high-quality SVG rendering (handles masks).
 *
 * Usage: node scripts/generate-icons.mjs
 * Output: ../../apps/desktop/src-tauri/icons/
 */

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { Resvg } from '@resvg/resvg-js';
import pngToIco from 'png-to-ico';
import sharp from 'sharp';

const __dirname = dirname(fileURLToPath(import.meta.url));
const LOGOS_DIR = join(__dirname, '..', 'src', 'logos');
const TAURI_ICONS_DIR = join(__dirname, '..', '..', '..', 'apps', 'desktop', 'src-tauri', 'icons');

// --- SVG Sources ---
const appIconSvg = readFileSync(join(LOGOS_DIR, 'app-icon.svg'), 'utf-8');
const appIconSmallSvg = readFileSync(join(LOGOS_DIR, 'app-icon-small.svg'), 'utf-8');

/**
 * Render an SVG string to a PNG buffer at the given pixel size.
 */
function renderSvgToPng(svgString, size) {
  const resvg = new Resvg(svgString, {
    fitTo: { mode: 'width', value: size },
  });
  const rendered = resvg.render();
  return rendered.asPng();
}

/**
 * Decide which SVG variant to use based on target size.
 * <=48px → small variant (thick bars), >48px → standard variant (thin bars).
 */
function getSvgForSize(size) {
  return size <= 48 ? appIconSmallSvg : appIconSvg;
}

// --- Target sizes for Tauri bundle ---
const targets = [
  // Tauri bundle icons
  { name: 'icon.png', size: 512 },
  { name: '32x32.png', size: 32 },
  { name: '128x128.png', size: 128 },
  { name: '128x128@2x.png', size: 256 },

  // Windows Store logos
  { name: 'Square30x30Logo.png', size: 30 },
  { name: 'Square44x44Logo.png', size: 44 },
  { name: 'Square71x71Logo.png', size: 71 },
  { name: 'Square89x89Logo.png', size: 89 },
  { name: 'Square107x107Logo.png', size: 107 },
  { name: 'Square142x142Logo.png', size: 142 },
  { name: 'Square150x150Logo.png', size: 150 },
  { name: 'Square284x284Logo.png', size: 284 },
  { name: 'Square310x310Logo.png', size: 310 },
  { name: 'StoreLogo.png', size: 50 },
];

async function main() {
  mkdirSync(TAURI_ICONS_DIR, { recursive: true });

  console.log('Generating PNG icons...');

  // Generate all PNG targets
  for (const { name, size } of targets) {
    const svg = getSvgForSize(size);
    const pngBuffer = renderSvgToPng(svg, size);
    const outPath = join(TAURI_ICONS_DIR, name);
    writeFileSync(outPath, pngBuffer);
    console.log(`  ✓ ${name} (${size}x${size})`);
  }

  // Generate ICO (16, 24, 32, 48, 256 embedded)
  console.log('Generating icon.ico...');
  const icoSizes = [16, 24, 32, 48, 256];
  const icoPngPaths = [];

  for (const size of icoSizes) {
    const svg = getSvgForSize(size);
    const pngBuffer = renderSvgToPng(svg, size);
    const tmpPath = join(TAURI_ICONS_DIR, `_ico_${size}.png`);
    writeFileSync(tmpPath, pngBuffer);
    icoPngPaths.push(tmpPath);
  }

  const icoBuffer = await pngToIco(icoPngPaths);
  writeFileSync(join(TAURI_ICONS_DIR, 'icon.ico'), icoBuffer);
  console.log('  ✓ icon.ico (16, 24, 32, 48, 256)');

  // Clean up temp ICO PNGs
  for (const tmpPath of icoPngPaths) {
    try { const { unlinkSync } = await import('fs'); unlinkSync(tmpPath); } catch {}
  }

  // Generate ICNS placeholder note
  // macOS ICNS requires icns tooling (iconutil on macOS). On Windows we generate
  // a 1024x1024 PNG that can be converted to ICNS on macOS via:
  //   mkdir icon.iconset && sips -z ... && iconutil -c icns icon.iconset
  const icnsPng = renderSvgToPng(appIconSvg, 1024);
  writeFileSync(join(TAURI_ICONS_DIR, 'icon-1024.png'), icnsPng);
  console.log('  ✓ icon-1024.png (for ICNS generation on macOS)');

  // Generate tray icons (small variant with bg + border-radius 4px)
  // Dark taskbar variant: #000 bg, white mark
  const trayDarkSvg = readFileSync(join(LOGOS_DIR, 'tray-icon.svg'), 'utf-8');
  const trayDarkPng = renderSvgToPng(trayDarkSvg, 32);
  writeFileSync(join(TAURI_ICONS_DIR, 'tray-icon.png'), trayDarkPng);
  console.log('  ✓ tray-icon.png (32x32, dark taskbar)');

  // Light taskbar variant: #f0f0f0 bg, #1a1a1a mark
  const trayLightSvg = readFileSync(join(LOGOS_DIR, 'tray-icon-light.svg'), 'utf-8');
  const trayLightPng = renderSvgToPng(trayLightSvg, 32);
  writeFileSync(join(TAURI_ICONS_DIR, 'tray-icon-light.png'), trayLightPng);
  console.log('  ✓ tray-icon-light.png (32x32, light taskbar)');

  console.log('\nDone! All icons generated in apps/desktop/src-tauri/icons/');
}

main().catch((err) => {
  console.error('Icon generation failed:', err);
  process.exit(1);
});
