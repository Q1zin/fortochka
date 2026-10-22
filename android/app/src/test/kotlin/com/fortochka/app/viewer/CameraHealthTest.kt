package com.fortochka.app.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CameraHealthTest {
    private val now = 1_800_000_000L

    @Test
    fun noFramesYet() {
        assertEquals("Камера ещё не прислала ни одного кадра", describeCamera(null, now))
    }

    @Test
    fun recentFrameMeansOnline() {
        assertTrue(describeCamera(now - 180, now).startsWith("Камера на связи"))
    }

    @Test
    fun longSilenceMeansOffline() {
        assertTrue(describeCamera(now - CAMERA_SILENT_AFTER_SECS - 1, now).startsWith("Камера молчит"))
    }

    @Test
    fun olderFrameShowsDate() {
        assertEquals("dd.MM HH:mm".length, clock(now - 2 * 24 * 3600, now).length)
        assertEquals("HH:mm".length, clock(now, now).length)
    }
}
