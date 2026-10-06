class Builder
  BASIC = [:a, :b].freeze

  def build
    acc = BASIC
    [1, 2].each { |n| acc += [n.to_s.to_sym] }
    acc
  end
end
