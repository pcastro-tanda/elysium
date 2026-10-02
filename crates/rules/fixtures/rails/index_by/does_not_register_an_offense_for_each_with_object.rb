x.each_with_object({}) { |el, h| h[el.to_sym] = foo(el) }
