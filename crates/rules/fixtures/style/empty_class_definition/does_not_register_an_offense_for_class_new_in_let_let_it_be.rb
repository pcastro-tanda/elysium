let(:application_mailer) { Class.new(ActionMailer::Base) }
let(:my_class) { Class.new }
let(:view_component) do
  Class.new(ViewComponent::Base)
end
let_it_be(:custom_config_class) { Class.new }
subject { Class.new(ViewComponent::Base) }
let(:my_class) { Class.new do
  def method
  end
end }
