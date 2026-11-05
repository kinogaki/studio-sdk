# Kinogaki Studio License Agreement

**Kinogaki Studio License Agreement**
Version 1.0, [date]

This Agreement is between Kinogaki Inc., a Delaware corporation, [address] ("Kinogaki", "we") and you, the individual or the organisation you act for ("you"). By installing, copying or using the Software, you accept this Agreement. If you do not accept it, do not install or use the Software.

## 1. Definitions
- **"Studio"** means the Kinogaki Studio application, including the plugins bundled with it, its documentation, and its updates.
- **"SDK"** means the Kinogaki Studio software development kit. It includes the `studio-sdk` source repository, the plugin kit published with each Studio release, the SDK documentation (including studiosdk.kinogaki.com), and the tools for building plugins.
- **"Sample Code"** means the source files in the SDK that are marked as sample code. This includes everything under `examples/`.
- **"Software"** means Studio and the SDK together.
- **"Plugin"** means software you create with the SDK that Studio loads. Plugins include node, format, kernel and render plugins.
- **"Licence Type"** means the edition, term and number of seats you obtained, as stated in your order or licence key: [editions to be defined, e.g. Personal/Indie/Studio].

## 2. Licence to use Studio
Subject to this Agreement and your Licence Type, Kinogaki grants you a non-exclusive, non-transferable licence. Under it, you may install and use Studio on the number of devices or seats your Licence Type allows, for the purposes it allows.

## 3. Licence to use the SDK
3.1 **Grant.** Kinogaki grants you a non-exclusive, non-transferable, royalty-free licence to use, copy and modify the SDK. The licence is only for developing, testing, supporting and distributing Plugins that run in a licensed copy of Studio.

3.2 **Public source is not open source.** The SDK's source is published so developers can read it, clone it and build against it. Public availability does not place the SDK under an open-source licence. Apart from Sample Code (section 4) and third-party components (section 8), the SDK stays under this Agreement.

3.3 **Your Plugins.**
- You own the Plugins you create.
- You may distribute them in compiled form under terms you choose, commercially or free, without paying Kinogaki any royalty.
- Your terms must not conflict with this Agreement.
- Users of your Plugin need their own licence for Studio.

3.4 **What may ship with a Plugin.** A Plugin may include the parts of the SDK that are compiled into it when it is built with the plugin kit. Apart from that, you may not redistribute the SDK or the plugin kit, except by linking to Kinogaki's official download or repository.

3.5 **Restrictions.** You may not use the SDK to:
- (a) build software that loads Plugins, or runs Studio's plugin interface, outside Studio;
- (b) build a product that substitutes for Studio;
- (c) work around licence checks, plugin loading checks or the kit identity checks.

## 4. Sample Code
Sample Code is licensed under the following notice, which also appears at the top of each sample file:

> Copyright (c) [year] Kinogaki Inc.
> Redistribution and use in source and binary forms, with or without modification, are permitted, provided that: (1) redistributions of source code retain this notice; (2) redistributions in binary form reproduce this notice in the documentation or other materials provided with the distribution; and (3) the Kinogaki name is not used to endorse or promote products derived from this software without prior written permission. THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.

## 5. Open-source plugins bundled with Studio
Some plugins bundled with Studio are open source under their own licences, for example the Pixel render engine (`studio-render-pixel`, dual-licensed Apache-2.0 or MIT). Those licences, not this Agreement, govern their source and binaries. Such a plugin builds against the SDK, and the SDK, including its render and viewport type crates, stays under this Agreement (owner, 2026-10-29). In practice the plugin is usable only with Studio. Other bundled plugins are provided in compiled form only, under section 2.

## 6. Ownership and general restrictions
6.1 Kinogaki and its licensors own the Software and all rights in it. All rights not expressly granted are reserved.

6.2 Except as this Agreement or applicable law allows, you may not:
- rent, lease, sublicense or resell Studio;
- remove proprietary notices;
- decompile or reverse engineer Studio's compiled code, except to the extent the law permits despite this restriction.

## 7. Trademarks
"Kinogaki" and "Kinogaki Studio" are trademarks of Kinogaki.
- You may say, factually, that your Plugin is "for Kinogaki Studio" or "compatible with Kinogaki Studio".
- You may not use our marks in a way that suggests we endorse your Plugin, or as part of your product's name or logo.

## 8. Third-party components
The Software includes third-party components under their own licences, such as the Rust standard library, wgpu and bytemuck. Those licences govern those components, and their notices are listed in THIRD_PARTY_NOTICES, shipped with Studio and the plugin kit.

## 9. Updates, compatibility and support
- Each Studio release publishes its own plugin kit. A Plugin must be built with the kit of the release it runs in, and Studio refuses Plugins built for another kit.
- We may change the SDK between releases.
- Support is provided only as your Licence Type states.

## 10. Feedback
If you send us suggestions or feedback, we may use them without obligation to you.

## 11. Term and termination
- This Agreement lasts until terminated.
- It ends automatically if you breach it and do not cure the breach within 30 days of notice.
- On termination you must stop using the Software. Plugins you distributed before termination remain yours.
- Sections 3.3 (ownership of Plugins), 4, 6, 7, 12, 13 and 15 survive.

## 12. Disclaimer of warranty
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NON-INFRINGEMENT, TO THE EXTENT THE LAW ALLOWS. [Edition-specific limited warranty, if any.]

## 13. Limitation of liability
TO THE EXTENT THE LAW ALLOWS:
- KINOGAKI IS NOT LIABLE FOR INDIRECT, INCIDENTAL, SPECIAL OR CONSEQUENTIAL DAMAGES, OR FOR LOST PROFITS, DATA OR BUSINESS;
- KINOGAKI'S TOTAL LIABILITY IS LIMITED TO THE AMOUNT YOU PAID FOR THE SOFTWARE IN THE 12 MONTHS BEFORE THE CLAIM, OR [USD 100] IF YOU PAID NOTHING.

## 14. Export
You will comply with applicable export control and sanctions laws.

## 15. General
- **Governing law:** the laws of the State of Delaware, without regard to conflict-of-laws rules, with exclusive jurisdiction in the state and federal courts located in Delaware.
- **Entire agreement:** this is the entire agreement about its subject matter.
- **Changes:** we may publish new versions for future releases; the version shipped with a release governs that release.
- **Severability:** if a provision is unenforceable, the rest remains in effect.
