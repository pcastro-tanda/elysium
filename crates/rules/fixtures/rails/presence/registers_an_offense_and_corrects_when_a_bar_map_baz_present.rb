a(:bar).map(&:baz).present? ? a(:bar).map(&:baz) : nil
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `a(:bar).map(&:baz).presence` instead of `a(:bar).map(&:baz).present? ? a(:bar).map(&:baz) : nil`.
