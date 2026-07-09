"""
Tests for enrichment.transcript module.

Closed by plan 17-02 (transcript.py + vtt_parser.py).
Tests: D-01 (fetch + yt-dlp fallback), D-03 (no captions skip),
D-04 (cache hit), sequential delay, manual-first (D-02).
"""
import pytest
from unittest.mock import MagicMock, patch, call


# ---------------------------------------------------------------------------
# Helper: build a minimal modules_raw structure with one lesson
# ---------------------------------------------------------------------------

def _make_modules(video_id="dQw4w9WgXcQ", title="Test Lesson"):
    return [
        {
            "module_num": 1,
            "module_title": "Module 1",
            "lessons": [
                {"lesson_num": 1, "title": title, "video_id": video_id},
            ],
        }
    ]


def _make_modules_two(video_id1="dQw4w9WgXcQ", video_id2="abc123ABCD"):
    return [
        {
            "module_num": 1,
            "module_title": "Module 1",
            "lessons": [
                {"lesson_num": 1, "title": "Lesson 1", "video_id": video_id1},
                {"lesson_num": 2, "title": "Lesson 2", "video_id": video_id2},
            ],
        }
    ]


# ---------------------------------------------------------------------------
# Test: cache hit skips API call (D-04)
# ---------------------------------------------------------------------------

def test_cache_hit_skips_fetch(tmp_cache_dir):
    """Cache hit: write a transcript file, assert API fetch() not called.

    Pre-condition: transcript already cached for video_id.
    Assert: fetch_and_cache_transcripts returns the cached text without
    calling YouTubeTranscriptApi.list() at all (call_count == 0).

    D-04 cross-run cache reuse. Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from enrichment.content_cache import write_transcript_cache

    video_id = "dQw4w9WgXcQ"
    cached_text = "This is the cached transcript."
    write_transcript_cache(video_id, cached_text)

    modules_raw = _make_modules(video_id)
    skipped = []

    with patch("enrichment.transcript.YouTubeTranscriptApi") as MockApi:
        result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    # Cache hit must NOT call the API at all
    MockApi.assert_not_called()
    assert result[video_id] == cached_text
    assert skipped == []


# ---------------------------------------------------------------------------
# Test: IpBlocked triggers yt-dlp fallback (D-01)
# ---------------------------------------------------------------------------

def test_ipblocked_triggers_ytdlp_fallback(tmp_cache_dir, monkeypatch):
    """IpBlocked on api.list() → yt-dlp subprocess called as fallback.

    Pre-condition: YouTubeTranscriptApi().list() raises IpBlocked.
    Assert: fetch_transcript_via_ytdlp is invoked; returned text stored.

    D-01 fallback. Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from youtube_transcript_api._errors import IpBlocked

    video_id = "dQw4w9WgXcQ"
    modules_raw = _make_modules(video_id)
    skipped = []

    # Build a mock API instance whose .list() raises IpBlocked
    mock_api_instance = MagicMock()
    mock_api_instance.list.side_effect = IpBlocked("blocked")

    ytdlp_text = "Fetched via yt-dlp fallback."

    with patch("enrichment.transcript.YouTubeTranscriptApi", return_value=mock_api_instance):
        with patch("enrichment.transcript.fetch_transcript_via_ytdlp", return_value=ytdlp_text) as mock_ytdlp:
            result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    # yt-dlp fallback must have been called with the video_id
    mock_ytdlp.assert_called_once_with(video_id)

    # Result must include the transcript from the fallback
    assert result.get(video_id) == ytdlp_text
    assert skipped == []


# ---------------------------------------------------------------------------
# Test: no captions → skipped list, not in transcripts dict (D-03)
# ---------------------------------------------------------------------------

def test_no_captions_skips_lesson(tmp_cache_dir):
    """TranscriptsDisabled → lesson in skipped list, not in transcripts dict.

    Pre-condition: video has no captions (TranscriptsDisabled raised).
    Assert: video_id absent from returned transcripts dict; video_id in
    skipped list; no exception propagates (D-03).

    Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from youtube_transcript_api._errors import TranscriptsDisabled

    video_id = "dQw4w9WgXcQ"
    modules_raw = _make_modules(video_id)
    skipped = []

    mock_api_instance = MagicMock()
    mock_api_instance.list.side_effect = TranscriptsDisabled(video_id)

    with patch("enrichment.transcript.YouTubeTranscriptApi", return_value=mock_api_instance):
        result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    # video_id must be absent from the result dict
    assert video_id not in result

    # video_id must appear in the skipped list
    assert any(video_id in str(entry) for entry in skipped), (
        f"Expected {video_id!r} in skipped list, got: {skipped}"
    )


# ---------------------------------------------------------------------------
# Test: sequential delay between non-cached fetches (Pitfall 4)
# ---------------------------------------------------------------------------

def test_sequential_delay_between_fetches(tmp_cache_dir, monkeypatch):
    """Sequential delay (delay parameter) between API fetches.

    Pre-condition: 2 lessons without cached transcripts.
    Assert: time.sleep is called between the two fetches (delay > 0).
    The delay parameter is passed directly; passing delay=0.1 lets us
    verify sleep is called without actually waiting.

    Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from youtube_transcript_api._errors import NoTranscriptFound

    video_id1 = "dQw4w9WgXcQ"
    video_id2 = "abc123ABCD"
    modules_raw = _make_modules_two(video_id1, video_id2)
    skipped = []

    # Build mock API that returns a snippet for each video_id
    def make_snippet(text):
        snip = MagicMock()
        snip.text = text
        return snip

    mock_transcript1 = MagicMock()
    mock_transcript1.fetch.return_value = [make_snippet("Transcript one.")]

    mock_transcript2 = MagicMock()
    mock_transcript2.fetch.return_value = [make_snippet("Transcript two.")]

    mock_list1 = MagicMock()
    mock_list1.find_manually_created_transcript.return_value = mock_transcript1

    mock_list2 = MagicMock()
    mock_list2.find_manually_created_transcript.return_value = mock_transcript2

    call_count = 0

    def list_side_effect(vid):
        nonlocal call_count
        call_count += 1
        return mock_list1 if vid == video_id1 else mock_list2

    mock_api_instance = MagicMock()
    mock_api_instance.list.side_effect = list_side_effect

    sleep_calls = []

    def fake_sleep(n):
        sleep_calls.append(n)

    with patch("enrichment.transcript.YouTubeTranscriptApi", return_value=mock_api_instance):
        with patch("enrichment.transcript.time") as mock_time:
            mock_time.sleep.side_effect = fake_sleep
            result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0.1)

    # sleep must have been called at least once between the two non-cached fetches
    assert len(sleep_calls) >= 1, (
        f"Expected time.sleep to be called at least once, got: {sleep_calls}"
    )

    # Both transcripts must be in the result
    assert video_id1 in result
    assert video_id2 in result


# ---------------------------------------------------------------------------
# Test: lesson without video_id is silently skipped
# ---------------------------------------------------------------------------

def test_lesson_without_video_id_skipped(tmp_cache_dir):
    """Lessons without a video_id are silently ignored.

    Pre-condition: lesson dict has no 'video_id' key (or it is falsy).
    Assert: no API call, no entry in result, no exception.

    Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts

    modules_raw = [
        {
            "module_num": 1,
            "module_title": "Module 1",
            "lessons": [
                {"lesson_num": 1, "title": "No Video Lesson"},
            ],
        }
    ]
    skipped = []

    with patch("enrichment.transcript.YouTubeTranscriptApi") as MockApi:
        result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    MockApi.assert_not_called()
    assert result == {}
    assert skipped == []


# ---------------------------------------------------------------------------
# Test: manual-first with auto fallback (D-02)
# ---------------------------------------------------------------------------

def test_manual_first_falls_back_to_auto(tmp_cache_dir):
    """Manual transcript preferred; NoTranscriptFound on manual → auto generated used.

    D-02 manual-first policy. Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from youtube_transcript_api._errors import NoTranscriptFound

    video_id = "dQw4w9WgXcQ"
    modules_raw = _make_modules(video_id)
    skipped = []

    snip = MagicMock()
    snip.text = "Auto-generated transcript text."

    mock_auto_transcript = MagicMock()
    mock_auto_transcript.fetch.return_value = [snip]

    mock_list = MagicMock()
    # Manual raises NoTranscriptFound → fallback to generated
    # NoTranscriptFound 1.x signature: (video_id, requested_language_codes, transcript_data)
    mock_list.find_manually_created_transcript.side_effect = NoTranscriptFound(
        video_id, ["en"], MagicMock()
    )
    mock_list.find_generated_transcript.return_value = mock_auto_transcript

    mock_api_instance = MagicMock()
    mock_api_instance.list.return_value = mock_list

    with patch("enrichment.transcript.YouTubeTranscriptApi", return_value=mock_api_instance):
        result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    # Generated transcript used as fallback
    assert result[video_id] == "Auto-generated transcript text."
    mock_list.find_generated_transcript.assert_called_once()


# ---------------------------------------------------------------------------
# Test: RequestBlocked also triggers yt-dlp fallback
# ---------------------------------------------------------------------------

def test_requestblocked_triggers_ytdlp_fallback(tmp_cache_dir):
    """RequestBlocked on api.list() → yt-dlp fallback (same as IpBlocked).

    D-01 fallback. Closed by 17-02 (transcript.py).
    """
    from enrichment.transcript import fetch_and_cache_transcripts
    from youtube_transcript_api._errors import RequestBlocked

    video_id = "dQw4w9WgXcQ"
    modules_raw = _make_modules(video_id)
    skipped = []

    mock_api_instance = MagicMock()
    mock_api_instance.list.side_effect = RequestBlocked("blocked")

    ytdlp_text = "Fetched via yt-dlp."

    with patch("enrichment.transcript.YouTubeTranscriptApi", return_value=mock_api_instance):
        with patch("enrichment.transcript.fetch_transcript_via_ytdlp", return_value=ytdlp_text) as mock_ytdlp:
            result = fetch_and_cache_transcripts(modules_raw, skipped, delay=0)

    mock_ytdlp.assert_called_once_with(video_id)
    assert result.get(video_id) == ytdlp_text


# ---------------------------------------------------------------------------
# Test: yt-dlp fallback subprocess mock
# ---------------------------------------------------------------------------

def test_fetch_transcript_via_ytdlp_calls_subprocess(tmp_cache_dir, tmp_path):
    """fetch_transcript_via_ytdlp invokes yt-dlp subprocess without shell=True.

    T-17-03 mitigation: argv list, no shell injection. Closed by 17-02.

    We pre-create a real VTT file in a real temp dir and patch subprocess.run
    so yt-dlp appears to have written it successfully (returncode=0). Then we
    also patch tempfile.TemporaryDirectory to redirect the tmp_dir path to our
    pre-populated real dir.
    """
    from pathlib import Path
    from enrichment.transcript import fetch_transcript_via_ytdlp

    video_id = "dQw4w9WgXcQ"
    fake_vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello world\n"

    # Write a fake .vtt file in our controlled real temp dir
    vtt_path = tmp_path / f"{video_id}.en.vtt"
    vtt_path.write_text(fake_vtt, encoding="utf-8")

    # Patch subprocess.run to return success without actually calling yt-dlp
    mock_run_result = MagicMock()
    mock_run_result.returncode = 0

    captured_cmd = []

    def fake_run(cmd, **kwargs):
        captured_cmd.extend(cmd)
        return mock_run_result

    # Patch TemporaryDirectory to return our real tmp_path
    class FakeTmpDir:
        def __enter__(self):
            return str(tmp_path)
        def __exit__(self, *args):
            pass

    with patch("enrichment.transcript.subprocess.run", side_effect=fake_run):
        with patch("enrichment.transcript.tempfile.TemporaryDirectory", return_value=FakeTmpDir()):
            text = fetch_transcript_via_ytdlp(video_id)

    # subprocess.run must have been called
    assert len(captured_cmd) > 0, "subprocess.run was not called"

    # Must NOT pass shell=True — our fake_run captures kwargs via subprocess.run patch;
    # verify the cmd list is present (list, not string) which is the no-shell invariant
    assert isinstance(captured_cmd[0], str), "Command should be a list of strings (no shell)"
    assert captured_cmd[0] == "yt-dlp", f"Expected yt-dlp as first element, got: {captured_cmd[0]}"

    # The URL must contain the video_id
    url_in_cmd = [arg for arg in captured_cmd if video_id in arg]
    assert url_in_cmd, f"video_id {video_id!r} not found in command: {captured_cmd}"

    # Result must contain the parsed VTT text
    assert text is not None
    assert "Hello world" in text
