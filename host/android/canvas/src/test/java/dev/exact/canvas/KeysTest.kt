package dev.exact.canvas

import android.view.KeyCharacterMap
import android.view.KeyEvent
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31], manifest = Config.NONE)
class KeysTest {
    private fun key(code: Int, meta: Int = 0) = KeyEvent(0, 0, KeyEvent.ACTION_DOWN, code, 0, meta)

    /** AOSP's German layout gives Q `ralt: '@'`: AltGr+Q types '@', as an `EditText` does. */
    @Test fun altGrTypesTheLayoutsThirdCharacter() {
        assertEquals('@'.code, Keys.typed('@'.code, 'q'.code, chord = false, alt = true))
        assertEquals('€'.code, Keys.typed('€'.code, 'e'.code, chord = false, alt = true))
    }

    /** A key the layout gives nothing with Alt is no character: Android's map answers 0, or the same character. */
    @Test fun altWithNoCharacterOfItsOwnTypesNothing() {
        assertEquals(0, Keys.typed(0, 'q'.code, chord = false, alt = true))
        assertEquals(0, Keys.typed('q'.code, 'q'.code, chord = false, alt = true))
    }

    @Test fun chordsAndDeadKeysTypeNothing() {
        assertEquals(0, Keys.typed('@'.code, 'q'.code, chord = true, alt = true))
        assertEquals(0, Keys.typed('c'.code, 'c'.code, chord = true, alt = false))
        assertEquals(0, Keys.typed('´'.code or KeyCharacterMap.COMBINING_ACCENT, 0, chord = false, alt = false))
    }

    @Test fun anEventTypesItsCharacterUnlessCtrlOrMetaIsHeld() {
        assertEquals('q'.code, Keys.typed(key(KeyEvent.KEYCODE_Q)))
        assertEquals('Q'.code, Keys.typed(key(KeyEvent.KEYCODE_Q, KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON)))
        assertEquals(0, Keys.typed(key(KeyEvent.KEYCODE_Q, KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON)))
        assertEquals(0, Keys.typed(key(KeyEvent.KEYCODE_Q, KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON)))
    }
}
