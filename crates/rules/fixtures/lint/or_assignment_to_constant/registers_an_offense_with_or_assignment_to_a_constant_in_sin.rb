def self.foo
  M::CONST ||= 1
           ^^^ Avoid using or-assignment with constants.
end
