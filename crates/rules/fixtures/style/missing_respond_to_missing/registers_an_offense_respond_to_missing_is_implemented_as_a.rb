class Test
  def self.respond_to_missing?
  end

  def method_missing
  ^^^^^^^^^^^^^^^^^^ When using `method_missing`, define `respond_to_missing?`.
  end
end
