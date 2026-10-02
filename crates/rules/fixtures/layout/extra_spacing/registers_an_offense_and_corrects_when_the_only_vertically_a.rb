foo do
  bar(:abcd) { it.qux }
end

foo do
  bar(:b) {  it }
           ^ Unnecessary spacing detected.
end
