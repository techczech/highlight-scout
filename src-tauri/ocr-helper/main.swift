import Foundation
import Vision
import AppKit

func ocr(_ path: String) -> String {
    guard let img = NSImage(contentsOfFile: path),
          let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return "" }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    let handler = VNImageRequestHandler(cgImage: cg, options: [:])
    do { try handler.perform([request]) } catch { return "" }
    guard let obs = request.results else { return "" }
    return obs.compactMap { $0.topCandidates(1).first?.string }.joined(separator: "\n")
}

let paths = Array(CommandLine.arguments.dropFirst())
var out: [String] = []
for p in paths {
    let t = ocr(p)
    if !t.isEmpty { out.append(t) }
}
print(out.joined(separator: "\n\n"))
