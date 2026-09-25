%w[x y z].select do
  self.axis == it
  ^^^^ Redundant `self` detected.
end
