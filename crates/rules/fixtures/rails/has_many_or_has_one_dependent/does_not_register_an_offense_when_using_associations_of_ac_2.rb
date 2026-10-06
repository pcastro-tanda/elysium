class User < ::ActiveResource::Base
  has_many :projects, class_name: 'API::Project'
end
