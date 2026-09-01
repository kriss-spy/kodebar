import "../../package/contents/ui"
import QtQuick
import QtTest

TestCase {
    id: testCase

    name: "ProviderIcon"
    when: windowShown
    width: 80
    height: 80

    ProviderIcon {
        id: providerIcon

        width: 64
        height: 64
        providerId: "opencode_zen"
    }

    function init() {
        providerIcon.providerId = "opencode_zen";
    }

    function test_knownProviderUsesPackagedIdentity() {
        compare(providerIcon.iconSource, "../images/provider-opencode.svg");
    }

    function test_unknownProviderUsesGenericIdentity() {
        providerIcon.providerId = "unknown";
        compare(providerIcon.iconSource, "view-statistics");
    }

    function test_packagedIdentityLoadsAsThemeMask() {
        tryCompare(providerIcon, "status", ProviderIcon.Ready, 2000);
        verify(providerIcon.paintedWidth > 0 && providerIcon.paintedHeight > 0,
               providerIcon.paintedWidth + "x" + providerIcon.paintedHeight);
        verify(providerIcon.valid);
        verify(providerIcon.isMask);
    }
}
