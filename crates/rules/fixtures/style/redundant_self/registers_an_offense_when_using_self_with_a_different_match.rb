case foo
  in Integer => bar
    self.bar + bar
  in Float => baz
    self.bar + baz
    ^^^^ Redundant `self` detected.
end
