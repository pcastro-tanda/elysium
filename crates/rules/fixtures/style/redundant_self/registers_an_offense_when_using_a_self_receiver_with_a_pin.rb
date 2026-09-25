foo = 17
case pattern
  in ^foo, *bar
    self.foo + self.bar + foo + bar
    ^^^^ Redundant `self` detected.
end
