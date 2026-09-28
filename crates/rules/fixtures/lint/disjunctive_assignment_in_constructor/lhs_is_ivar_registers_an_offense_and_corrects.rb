class Banana
  def initialize
    @delicious ||= true
               ^^^ Unnecessary disjunctive assignment. Use plain assignment.
  end
end
