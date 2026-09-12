/* Real, dedicated non-Python test harness for tre-ffi (IMPLEMENTATION.md
 * Phase 10 Step 10.3 task 4) -- exercises the real extern "C" entry
 * points directly, not through cargo test's own Rust harness (that just
 * compiles, links, and runs this program -- see ../c_harness.rs).
 *
 * Builds a registry, inserts a solid red rectangle, renders it headlessly,
 * and asserts real pixel bytes at both the rectangle's center and a
 * background pixel outside it -- not just "the call didn't crash." */

#include <stdio.h>
#include <stdlib.h>

#include "tre_ffi.h"

#define WIDTH 64u
#define HEIGHT 64u
#define RECT_X 10.0f
#define RECT_Y 10.0f
#define RECT_W 30.0f
#define RECT_H 30.0f

static int fail(const char *message) {
    fprintf(stderr, "FAIL: %s\n", message);
    return 1;
}

int main(void) {
    TreShapeRegistry registry = tre_shape_registry_new();
    if (registry == NULL) {
        return fail("tre_shape_registry_new returned a null registry");
    }
    if (tre_shape_registry_len(registry) != 0) {
        return fail("a freshly created registry is not empty");
    }

    uint32_t red = tre_rgba8(255, 0, 0, 255);
    TreShapeId rect_id = NULL;
    TreErrorCode code = tre_shape_registry_insert_rectangle(
        registry, RECT_X, RECT_Y, RECT_W, RECT_H, red, &rect_id);
    if (code != TRE_ERROR_CODE_SUCCESS) {
        return fail("tre_shape_registry_insert_rectangle did not succeed");
    }
    if (rect_id == NULL) {
        return fail("tre_shape_registry_insert_rectangle left out_id null on success");
    }
    if (tre_shape_registry_len(registry) != 1) {
        return fail("registry length is not 1 after inserting one rectangle");
    }

    /* Reject an invalid (negative) dimension rather than silently
     * accepting or crashing -- proves the validation this crate adds on
     * top of the always-succeeding Rust ShapeRegistry::insert. */
    TreShapeId bad_id = NULL;
    code = tre_shape_registry_insert_rectangle(registry, 0.0f, 0.0f, -1.0f, 10.0f, red, &bad_id);
    if (code != TRE_ERROR_CODE_INVALID_ARGUMENT) {
        return fail("a negative width was not rejected as TRE_ERROR_CODE_INVALID_ARGUMENT");
    }
    if (bad_id != NULL) {
        return fail("out_id was not left null after a rejected insert");
    }

    TreHeadlessRenderer renderer = NULL;
    code = tre_headless_renderer_new(WIDTH, HEIGHT, &renderer);
    if (code != TRE_ERROR_CODE_SUCCESS || renderer == NULL) {
        fprintf(stderr, "tre_headless_renderer_new failed with code %d\n", (int)code);
        return fail("could not create a headless renderer");
    }

    TreFrameBuffer frame;
    code = tre_headless_renderer_render(renderer, registry, &frame);
    if (code != TRE_ERROR_CODE_SUCCESS) {
        fprintf(stderr, "tre_headless_renderer_render failed with code %d\n", (int)code);
        tre_headless_renderer_free(renderer);
        tre_shape_registry_free(registry);
        return fail("rendering the registry did not succeed");
    }

    int exit_code = 0;
    size_t expected_len = (size_t)WIDTH * (size_t)HEIGHT * 4u;
    if (frame.len != expected_len) {
        fprintf(stderr, "expected %zu bytes, got %zu\n", expected_len, frame.len);
        exit_code = fail("frame buffer length does not match width * height * 4");
    } else {
        /* Raw BGRA8 (HEADLESS_FORMAT), matching the engine's own real
         * internal readback format -- see crate::frame_buffer's module
         * doc comment. A pure, fully-saturated red rectangle round-trips
         * exactly through the pipeline's sRGB encode/decode (0 and 255
         * are the transfer function's own fixed points), so this checks
         * exact bytes, not an approximate/tolerance comparison. */
        size_t cx = (size_t)(RECT_X + RECT_W / 2.0f);
        size_t cy = (size_t)(RECT_Y + RECT_H / 2.0f);
        size_t center_offset = (cy * WIDTH + cx) * 4u;
        const uint8_t *center = frame.data + center_offset;
        if (center[0] != 0 || center[1] != 0 || center[2] != 255 || center[3] != 255) {
            fprintf(stderr, "center pixel BGRA = [%u, %u, %u, %u], expected [0, 0, 255, 255]\n",
                    center[0], center[1], center[2], center[3]);
            exit_code = fail("rectangle center pixel is not the expected red");
        }

        /* A background pixel well outside the rectangle must differ from
         * the rectangle's own fill -- proves the render actually drew
         * something spatially bounded, not a full-frame clear. */
        size_t bg_offset = (size_t)(HEIGHT - 1) * WIDTH * 4u + (WIDTH - 1) * 4u;
        const uint8_t *background = frame.data + bg_offset;
        if (background[0] == 0 && background[1] == 0 && background[2] == 255 &&
            background[3] == 255) {
            exit_code = fail("background pixel matches the rectangle's own fill color");
        }
    }

    tre_frame_buffer_free(frame);
    tre_headless_renderer_free(renderer);
    tre_shape_registry_free(registry);

    if (exit_code == 0) {
        printf("tre-ffi C harness: all checks passed\n");
    }
    return exit_code;
}
