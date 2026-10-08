class User < T::Struct
  def configure
    prop :methodName, String
  end

  class Profile
    prop :displayName, String
  end

  module Settings
    const :themeName, String
  end

  class << self
    prop :singletonName, String
  end
end
