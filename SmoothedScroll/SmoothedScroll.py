import math
from time import perf_counter
from typing import Union, Callable

from .models import SmoothedScrollConfig, ScrollConfig, ScrollEvent
from .utils import Timer, MouseListener, get_display_frequency, set_console_ctrl_handler, scroll

class SmoothedScroll:
    def __init__(self, config: SmoothedScrollConfig):
        self._pulse_normalize = 1
        self._timer = Timer(daemon=True)
        self._listener = MouseListener(
            callback=self.scroll,
            config=config,
            daemon=True
        )
        self._display_frequency = get_display_frequency()
        self._refresh_rate = (1000 / self._display_frequency - 0.3) / 1000
        self._queue = []
        self._pending = False
        self._previous_scroll_time = 0
        self._excess_delta_x = 0
        self._excess_delta_y = 0

    def start(self, is_block: bool = True):
        self._timer.start()
        self._listener.start()
        set_console_ctrl_handler(lambda _: self.join())
        if is_block:
            self._listener.listen()

    def scroll(self, delta: Union[int, float], is_horizontal: bool, config: ScrollConfig) -> None:
        delta = math.copysign(config.distance, delta) if config.distance else delta
        current_time = perf_counter()

        if (elapsed := current_time - self._previous_scroll_time) < config.acceleration_delta:
            acceleration = config.opposite_acceleration if delta > 0 else config.acceleration
            factor = (1 + 0.05 / elapsed) / 2 * acceleration
            if factor > 1:
                delta *= min(factor, config.acceleration_max)

        self._previous_scroll_time = current_time
        self._queue.append(ScrollEvent(delta, is_horizontal, config))

        if not self._pending:
            self._request_scroll()

    def _request_scroll(self):
        def request_scroll():
            delta_x, delta_y = 0, 0

            for scroll_event in self._queue[:]:  # iterate over a copy since we might remove items
                elapsed = perf_counter() - scroll_event.start
                finished = elapsed >= scroll_event.config.duration
                progress = self._pulse(
                    1 if finished else elapsed / scroll_event.config.duration,
                    scroll_event.config.pulse_scale
                )

                delta = scroll_event.ease(progress) - scroll_event.previous_delta
                
                if scroll_event.is_horizontal:
                    delta_y += delta
                else:
                    delta_x += delta
                
                scroll_event.previous_delta += delta

                if finished:
                    self._queue.remove(scroll_event)

            excess_x, delta_x = math.modf(delta_x)
            self._excess_delta_x, extra_x = math.modf(self._excess_delta_x + excess_x)

            excess_y, delta_y = math.modf(delta_y)
            self._excess_delta_y, extra_y = math.modf(self._excess_delta_y + excess_y)

            if (int_delta_x := int(delta_x + extra_x)):
                scroll(int_delta_x, False)
            if (int_delta_y := int(delta_y + extra_y)):
                scroll(int_delta_y, True)

            if self._queue:
                return self.__request_frame(request_scroll, self._refresh_rate)

            self._excess_delta_x = self._excess_delta_y = 0
            self._pending = False

        self.__request_frame(request_scroll, 0)
        self._pending = True

    def __request_frame(self, callback: Callable, timeout: Union[int, float]) -> None:
        self._timer.set_timeout(callback, timeout)

    def _pulse(self, x: Union[int, float], scale: Union[int, float]):
        if x >= 1:
            return 1
        if x <= 0:
            return 0

        if self._pulse_normalize == 1:
            self._pulse_normalize /= self.__pulse(1, scale)

        return self.__pulse(x, scale)

    def __pulse(self, x: Union[int, float], scale: Union[int, float]):
        x_scaled = x * scale
        if x_scaled < 1:
            val = x_scaled - (1 - math.exp(-x_scaled))
        else:
            start = math.exp(-1)
            val = start + ((1 - math.exp(-x_scaled + 1)) * (1 - start))

        return val * self._pulse_normalize

    def join(self) -> None:
        self._listener.join()
        self._timer.join()

    def get_config(self) -> SmoothedScrollConfig:
        return self._listener.config

    def update_config(self, config: SmoothedScrollConfig) -> None:
        self._listener.config = config