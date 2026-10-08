# typed: strong
class Report
  CATEGORIES = ["a", "b"].freeze

  def all
    [CATEGORIES].flatten
  end
end
