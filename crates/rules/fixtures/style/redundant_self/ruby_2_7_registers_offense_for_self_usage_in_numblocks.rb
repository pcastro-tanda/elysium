%w[x y z].select do
  self.axis == _1
  ^^^^ Redundant `self` detected.
end
