stub_const('FakeService', Class.new)

stub_const("MicrosoftSync::TestErrorNotPublic", Class.new(StandardError))

stub_const("MicrosoftSync::TestError",
           Class.new(MicrosoftSync::Errors::PublicError) do
             def self.public_message
               I18n.t "oops, this is a public error"
             end
           end)
