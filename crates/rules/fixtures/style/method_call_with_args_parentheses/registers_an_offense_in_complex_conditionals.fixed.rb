def foo
  if cond.present? && verify?(:something)
    h.do_with kw: value
  elsif cond.present? || verify?(:something_else)
    h.do_with kw: value
  elsif whatevs?
    h.do_with kw: value
  end
end
