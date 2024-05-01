%global __strip /bin/true
%define _binaries_in_noarch_packages_terminate_build 0
%define _unpackaged_files_terminate_build 0

Name: aerospike-php-client
Version: %{?VERSION}
Release: 1
Summary: Aerospike PHP Client Library
License: Aerospike, Inc.
Group: Applications/Databases
BuildArch: noarch
URL: https://github.com/aerospike/php-client
Source0: aerospike-php-client-%{?VERSION}.tar.gz
# Add any additional Source lines if needed

%description
The Aerospike PHP client library enables you to build applications using PHP to interact with Aerospike databases.

%prep
%setup -q -n aerospike-php-client-1.0.2

%build
# No build step required for PHP libraries

%install


rm -rf $RPM_BUILD_ROOT
mkdir -p $RPM_BUILD_ROOT%{_bindir}
mkdir -p $RPM_BUILD_ROOT/etc
mkdir -p $RPM_BUILD_ROOT%{_libdir}

# Copy binary files
install -m 755 asld $RPM_BUILD_ROOT%{_bindir}/asld

# Copy configuration file
install -m 644 asld.toml $RPM_BUILD_ROOT/etc/asld.toml

# Copy library file
install -m 755 libaerospike_php.so $RPM_BUILD_ROOT%{_libdir}/libaerospike_php.so

# Copy scripts
install -m 755 postinst $RPM_BUILD_ROOT%{_libdir}/postinst


%files
%{_bindir}/asld
/etc/asld.toml
%{_libdir}/libaerospike_php.so
%{_libdir}/postinst

%post
# Post-installation script
%{_libdir}/postinst
echo "Aerospike PHP Client installed successfully."

%postun
# Post-uninstallation script
rm -f %{_libdir}/libaerospike_php.so
rm -f %{_libdir}/postinst
echo "Aerospike PHP Client removed."

%changelog

