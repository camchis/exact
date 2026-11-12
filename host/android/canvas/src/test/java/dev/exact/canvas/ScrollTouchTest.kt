package dev.exact.canvas

import android.app.Activity
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.widget.FrameLayout
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.annotation.LooperMode
import org.robolectric.shadow.api.Shadow

/**
 * A finger on the reader's view, through [ExactView.onTouchEvent] as the platform delivers it: [Scrolling], the
 * hosted fields' dispatcher ([Fields]) and what reaches exact2, with exact2's answer for the touch's down
 * ([Native.under]) given when the test says, as a busy exact2 thread gives it late. Each gesture is played twice,
 * answered at once and answered after the finger has done all it does, and must scroll the same.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31], manifest = Config.NONE, shadows = [ScrollTouchTest.Thread::class, ScrollTouchTest.Host::class])
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@LooperMode(LooperMode.Mode.PAUSED)
class ScrollTouchTest {
    /** exact2's thread: what the reader sends it, and the answers it owes. */
    @Implements(Owner::class, isInAndroidSdk = false)
    class Thread {
        /** Each touch exact2 heard: 0 down, 1 up, 2 move, 3 cancel. */
        val touches = ArrayList<Int>()
        /** How far the reader moved each scroller, by id. */
        val moved = HashMap<Int, Float>()
        val answers = ArrayDeque<(IntArray) -> Unit>()
        @Implementation fun __constructor__(hwbuf: Boolean, fast: Boolean, priority: Int, intent: (String) -> String?) {}
        @Implementation fun under(x: Float, y: Float, done: (IntArray) -> Unit) { answers.addLast(done) }
        @Implementation fun touch(action: Int, x: Float, y: Float, tool: Int, waitMs: Long): Boolean { touches.add(action); return false }
        @Implementation fun finger(action: Int, id: Int, x: Float, y: Float, waitMs: Long) = false
        @Implementation fun scrolled(id: Int, dx: Float, dy: Float, rev: Int, seq: Int) { moved[id] = (moved[id] ?: 0f) + dx + dy }
        @Implementation fun answer(waitMs: Long, f: () -> Unit) = false
        @Implementation fun call(f: () -> Unit) {}
        @Implementation fun input() {}
        @Implementation fun held(on: Boolean) {}
        @Implementation fun running(on: Boolean) {}
        @Implementation fun timerReader(view: Any) {}
        @Implementation fun timerReport(view: Any, scrolling: Boolean, sampling: Boolean, applied: Long, began: Long, ended: Long) {}
    }

    @Implements(Native::class, isInAndroidSdk = false)
    class Host {
        companion object {
            @JvmStatic @Implementation fun __staticInitializer__() {}
            @JvmStatic @Implementation fun fieldInput(id: Int, generation: Int) {}
            @JvmStatic @Implementation fun fieldFocused(id: Int, generation: Int, focused: Boolean) {}
            @JvmStatic @Implementation fun fieldSelected(id: Int, generation: Int, start: Int, end: Int) {}
        }
    }

    private lateinit var activity: Activity
    private lateinit var view: ExactView
    private lateinit var thread: Thread
    private val slop get() = ViewConfiguration.get(activity).scaledTouchSlop
    private var time = 0L

    @Before fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        activity.intent.putExtra("warm", "0")
        view = ExactView(activity, false) {}
        activity.setContentView(FrameLayout(activity).also { it.addView(view, FrameLayout.LayoutParams(400, 800)) })
        val owner = Owner(false, false, 0) { null }
        thread = Shadow.extract(owner)
        view.owner = owner
        view.ownerStarted(true)
    }

    /** A vertical scroller [id] at [offset] of [max], its port the view's top 400 by 800. */
    private fun scroller(id: Int, offset: Float, max: Float = 500f) =
        intArrayOf(id, 2, 0, 0, offset.toBits(), 0, max.toBits(), 0, 0, 400f.toBits(), 800f.toBits())

    /** exact2's answer: no claims, these scrollers innermost first, [field] under the touch. */
    private fun reply(vararg scrollers: IntArray, field: Int = 0) =
        intArrayOf(0, scrollers.size, field) + scrollers.fold(IntArray(0)) { a, s -> a + s }

    private fun answer(words: IntArray) = thread.answers.removeFirst()(words)

    private fun touch(action: Int, vararg fingers: Pair<Float, Float>, index: Int = 0) {
        val props = Array(fingers.size) { k -> MotionEvent.PointerProperties().apply { id = k; toolType = MotionEvent.TOOL_TYPE_FINGER } }
        val coords = Array(fingers.size) { k -> MotionEvent.PointerCoords().apply { x = fingers[k].first; y = fingers[k].second } }
        val e = MotionEvent.obtain(0, time, action or (index shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), fingers.size,
            props, coords, 0, 0, 1f, 1f, 0, 0, android.view.InputDevice.SOURCE_TOUCHSCREEN, 0)
        time += 10
        view.dispatchTouchEvent(e)
        e.recycle()
    }
    private fun down(y: Float) = touch(MotionEvent.ACTION_DOWN, 100f to y)
    private fun move(y: Float) = touch(MotionEvent.ACTION_MOVE, 100f to y)
    private fun up(y: Float) = touch(MotionEvent.ACTION_UP, 100f to y)

    /** [gesture] from a down at y 400, its answer [words] given at once and then given after it: what each scrolled,
     * by scroller, and what exact2 heard. */
    private fun both(words: IntArray, gesture: () -> Unit): List<Pair<Map<Int, Float>, List<Int>>> = listOf(true, false).map { soon ->
        setUp()
        down(400f)
        if (soon) answer(words)
        gesture()
        if (!soon) answer(words)
        HashMap(thread.moved) to ArrayList(thread.touches)
    }

    @Test fun aLateAnswerTakesTheWayTheFingerWentPastTheSlop() {
        val (soon, late) = both(reply(scroller(1, 0f))) { move(390f); move(370f); move(350f) }
        assertEquals(50f - slop, soon.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
        assertTrue(slop in 11..29)
        assertEquals("answered at once, exact2 hears the move within the slop", listOf(0, 2, 3), soon.second)
        assertEquals("answered late, it heard the moves until the scroll took the touch", listOf(0, 2, 2, 2, 3), late.second)
    }

    @Test fun aFingerThatLiftedBeforeTheAnswerScrollsAndFlings() {
        val (soon, late) = both(reply(scroller(1, 0f))) { move(380f); move(340f); move(300f); up(300f) }
        assertEquals(100f - slop, late.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
        assertTrue("the lift flings", view.scrolling().timerActive)
        // exact2 heard the lift before the touch was known to be a scroll: it has nothing to cancel.
        assertEquals(listOf(0, 2, 2, 2, 1), late.second)
    }

    @Test fun theUpsOwnSampleMovesNothing() {
        // The platform's lists take a lift's position into its velocity, never into the drag.
        val (soon, late) = both(reply(scroller(1, 0f))) { move(380f); move(350f); up(320f) }
        assertEquals(50f - slop, late.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
    }

    @Test fun aTurnAtTheEndIsNotLostToTheNetWay() {
        // Down past the top first, which nothing takes, then up past where the finger began: the scroll is the way
        // up from the turn, as it is live, not the way up from the down.
        val (soon, late) = both(reply(scroller(1, 0f))) { move(420f); move(440f); move(400f); move(360f) }
        assertEquals(80f, late.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
    }

    @Test fun aTurnHandsOffBetweenNestedScrollersInOrder() {
        // The inner list is at its top and the outer one 100 down: down goes to the outer one, up to the inner.
        val words = reply(scroller(1, 0f), scroller(2, 100f))
        val (soon, late) = both(words) { move(420f); move(440f); move(400f); move(360f) }
        assertEquals(-(40f - slop), late.first[2]!!, 0.01f)
        assertEquals(80f, late.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
    }

    @Test fun aSecondFingerTakesOverFromAFirstThatLiftedBeforeTheAnswer() {
        val (soon, late) = both(reply(scroller(1, 0f))) {
            touch(MotionEvent.ACTION_POINTER_DOWN, 100f to 400f, 200f to 400f, index = 1)
            touch(MotionEvent.ACTION_POINTER_UP, 100f to 400f, 200f to 400f, index = 0)
            // The second finger is all that is down: it is pointer 1, at index 0.
            secondFinger(MotionEvent.ACTION_MOVE, 360f)
            secondFinger(MotionEvent.ACTION_MOVE, 340f)
        }
        assertEquals(60f - slop, late.first[1]!!, 0.01f)
        assertEquals(soon.first, late.first)
        // The drag has a finger: it goes on moving the list, and its lift ends it.
        secondFinger(MotionEvent.ACTION_MOVE, 300f)
        assertEquals(100f - slop, thread.moved[1]!!, 0.01f)
        secondFinger(MotionEvent.ACTION_UP, 300f)
        assertFalse(view.scrolling().dragging())
    }

    private fun secondFinger(action: Int, y: Float) {
        val props = arrayOf(MotionEvent.PointerProperties().apply { id = 1; toolType = MotionEvent.TOOL_TYPE_FINGER })
        val coords = arrayOf(MotionEvent.PointerCoords().apply { x = 200f; this.y = y })
        val e = MotionEvent.obtain(0, time, action, 1, props, coords, 0, 0, 1f, 1f, 0, 0, android.view.InputDevice.SOURCE_TOUCHSCREEN, 0)
        time += 10
        view.dispatchTouchEvent(e)
        e.recycle()
    }

    @Test fun anAnswerAfterTheActivityStoppedMovesNothing() {
        down(400f); move(380f); move(340f); move(300f); up(300f)
        view.running(false)
        answer(reply(scroller(1, 0f)))
        assertNull(thread.moved[1])
        assertFalse("no fling in a stopped window", view.scrolling().timerActive)
    }

    @Test fun anAnswerAfterTheViewLeftItsWindowMovesNothing() {
        down(400f); move(380f); move(340f); move(300f); up(300f)
        (view.parent as FrameLayout).removeView(view)
        answer(reply(scroller(1, 0f)))
        assertNull(thread.moved[1])
        assertFalse(view.scrolling().timerActive)
    }

    // A hosted text field under the finger ([Fields]): the touch is held for it until the answer.

    private fun field(keeps: Boolean): ArrayList<Int> {
        val fields = Fields(activity)
        (view.parent as FrameLayout).addView(fields)
        view.fields = fields
        fields.view = view
        fields.told(FieldsTest.statement(x = 50f, y = 380f, w = 100f, h = 40f))
        val got = ArrayList<Int>()
        fields.text(1)!!.setOnTouchListener { v, e ->
            got.add(e.actionMasked)
            // A field that drags its selection asks the views around it to leave it the touch.
            if (keeps && e.actionMasked == MotionEvent.ACTION_MOVE) v.parent.requestDisallowInterceptTouchEvent(true)
            false
        }
        return got
    }

    @Test fun aFieldGetsWhatItHeldUntilTheScrollTakesIt() {
        for (soon in listOf(true, false)) {
            setUp()
            val got = field(keeps = false)
            down(400f)
            if (soon) answer(reply(scroller(1, 0f), field = 1))
            move(396f); move(380f); move(360f); up(360f)
            if (!soon) answer(reply(scroller(1, 0f), field = 1))
            assertEquals("answered at once: $soon", listOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE, MotionEvent.ACTION_CANCEL), got)
            assertEquals(40f - slop, thread.moved[1]!!, 0.01f)
            assertEquals("exact2 heard nothing of the field's touch", emptyList<Int>(), thread.touches)
        }
    }

    @Test fun aFieldThatKeepsItsTouchKeepsItWhenTheAnswerIsLate() {
        for (soon in listOf(true, false)) {
            setUp()
            val got = field(keeps = true)
            down(400f)
            if (soon) answer(reply(scroller(1, 0f), field = 1))
            move(396f); move(380f); move(360f); up(360f)
            if (!soon) answer(reply(scroller(1, 0f), field = 1))
            assertEquals("answered at once: $soon", listOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE, MotionEvent.ACTION_MOVE,
                MotionEvent.ACTION_MOVE, MotionEvent.ACTION_UP), got)
            assertNull(thread.moved[1])
        }
    }

    @Test fun aTouchInAFieldTheAnswerDoesNotGiveItGoesToExact2UntilTheScrollTakesIt() {
        val got = field(keeps = false)
        down(400f); move(396f); move(380f); move(360f); up(360f)
        answer(reply(scroller(1, 0f), field = 0))
        assertEquals(emptyList<Int>(), got)
        assertEquals(listOf(0, 2, 3), thread.touches)
        assertEquals(40f - slop, thread.moved[1]!!, 0.01f)
    }

    private fun ExactView.scrolling(): Scrolling =
        ExactView::class.java.getDeclaredField("scrolling").apply { isAccessible = true }.get(this) as Scrolling

    private fun Scrolling.dragging(): Boolean =
        Scrolling::class.java.getDeclaredField("mode").apply { isAccessible = true }.getInt(this) == 3
}
