class SomeClass
  EXPIRIES = {
    yearly: Proc.new { 1.year.ago },
    monthly: Proc.new { 1.month.ago }
  }
end
