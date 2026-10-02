def foo
  if cond.present? && verify?(:something)
    h.do_with(kw: value)
             ^^^^^^^^^^^ Omit parentheses for method calls with arguments.
  elsif cond.present? || verify?(:something_else)
    h.do_with(kw: value)
             ^^^^^^^^^^^ Omit parentheses for method calls with arguments.
  elsif whatevs?
    h.do_with(kw: value)
             ^^^^^^^^^^^ Omit parentheses for method calls with arguments.
  end
end
