"foo".try(:to_f)
^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `:to_f`, use stricter `{ |i| Float(i) }`.
