if something
  self.foo ||= default
  ^^^^^^^^^^^^^^^^^^^^ Move `self.foo ||= default` out of the conditional.
  do_x
else
  self.foo ||= default
  ^^^^^^^^^^^^^^^^^^^^ Move `self.foo ||= default` out of the conditional.
  do_y
end
