serializer = users.respond_to?(:each) ? :each_serializer : :serializer
render json: users, serializer => UserSerializer
