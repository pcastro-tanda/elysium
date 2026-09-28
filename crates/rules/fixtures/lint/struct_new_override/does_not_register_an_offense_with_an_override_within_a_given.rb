Good = Struct.new(:id, :name) do
  def members
    super.tap { |ret| pp "members: " + ret.to_s }
  end
end
