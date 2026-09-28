class Banana
  def initialize
    # With the limitations of static analysis, it's very difficult
    # to determine, after this method call, whether the disjunctive
    # assignment is necessary or not.
    absolutely_any_method
    @delicious ||= true
  end
end
