%w[draft preview moderation approved rejected].each do |state|
  self.state == state
  define_method "#{state}?" do
    self.state == state
  end
end
