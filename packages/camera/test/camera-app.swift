// The system camera UI's answer on iOS (`takePhotoWithCameraApp`): compiled
// with @exact/camera's Apple sources for the iOS simulator by apple.mjs and run
// there with `simctl spawn`, since the simulator has no camera to show the UI
// with. The picker's `info` for a photo taken with the phone held upright (the
// sensor's pixels landscape, the image `.right`) becomes a JPEG whose `Photo`
// is portrait; `info` with no image is `UnknownError`.
import ExactPackage
import Foundation
import ImageIO
import UIKit

@main
struct CameraAppTest {
    static func main() {
        var failed = false
        func check(_ name: String, _ ok: Bool, _ detail: @autoclosure () -> String) {
            if !ok { failed = true }
            print("\(ok ? "PASS" : "FAIL") apple-camera-app \(name)\(ok ? "" : ": \(detail())")")
        }

        let sensor = UIGraphicsImageRenderer(size: CGSize(width: 40, height: 30), format: {
            let f = UIGraphicsImageRendererFormat(); f.scale = 1; return f
        }()).image { ctx in UIColor.orange.setFill(); ctx.fill(CGRect(x: 0, y: 0, width: 40, height: 30)) }.cgImage!
        let held = UIImage(cgImage: sensor, scale: 1, orientation: .right)
        let metadata: [String: Any] = [kCGImagePropertyExifDictionary as String: [kCGImagePropertyExifLensModel as String: "back camera"]]
        switch CameraAppDelegate.kept([.originalImage: held, .mediaMetadata: metadata]) {
        case .success(let data):
            let photo = CameraModule.photo(path: "app:/tmp/camera/1.jpg", data: data)
            check("a kept photo is a JPEG", photo["type"] as? String == "image/jpeg", "\(photo)")
            check("sized as it is shown, upright", photo["width"] as? Int == 30 && photo["height"] as? Int == 40, "\(photo)")
            let props = CGImageSourceCreateWithData(data as CFData, nil).flatMap { CGImageSourceCopyPropertiesAtIndex($0, 0, nil) } as? [CFString: Any]
            let exif = props?[kCGImagePropertyExifDictionary] as? [CFString: Any]
            check("the camera's metadata is kept", exif?[kCGImagePropertyExifLensModel] as? String == "back camera", "\(String(describing: props))")
        case .failure(let f):
            check("a kept photo is a JPEG", false, f.code)
        }
        if case .failure(let f) = CameraAppDelegate.kept([:]) { check("no image is UnknownError", f.code == "UnknownError", f.code) } else { check("no image is UnknownError", false, "an image") }

        var answers: [String] = []
        let delegate = CameraAppDelegate { result in
            if case .failure(let f) = result { answers.append(f.code) } else { answers.append("photo") }
        }
        delegate.finish(.failure(PhotoFailure("AbortError")))
        delegate.finish(.failure(PhotoFailure("UnknownError")))
        check("a cancel is AbortError, answered once", answers == ["AbortError"], "\(answers)")
        exit(failed ? 1 : 0)
    }
}
