define_method :foo do
  M::CONST ||= 1
           ^^^ Avoid using or-assignment with constants.
end
