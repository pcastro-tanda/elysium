module CopTest
  def self.test
    if something && something_else
    ^^ Use a guard clause (`return unless something && something_else`) instead of wrapping the code inside a conditional expression.
      work
    end
  end
end
